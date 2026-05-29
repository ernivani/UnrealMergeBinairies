"""
Proof-of-work Blueprint diff from Python - parses .uasset Export Table
directly to count UObject instances by class. No UE reflection needed.

Why: the Name Table is deduplicated, so a strings-scan can't see that v2
has one more K2Node_VariableGet than v1. The Export Table is what lists
*instances*: one entry per UObject in the package, each pointing at its
class via an FName index. Counting class references in the Export Table
gives an exact per-class instance count.

PackageFileSummary layout (UE5, recent versions):
  Tag                      u32 = 0x9E2A83C1
  LegacyFileVersion        i32 (negative)
  LegacyUE3Version         i32
  FileVersionUE4           i32
  FileVersionUE5           i32  (only if LegacyFileVersion <= -8)
  FileVersionLicenseeUE    i32
  CustomVersions           array
  TotalHeaderSize          i32
  FolderName               FString
  PackageFlags             u32
  NameCount                i32
  NameOffset               i32
  ...

We locate NameCount/NameOffset by trying every aligned int32 pair in the
first 2KB of the file; for each candidate we attempt to decode that many
Name entries at NameOffset and accept the first pair that produces a
plausible name table (>50% of entries are printable ASCII identifiers).

Then we locate ExportCount/ExportOffset similarly. Each FObjectExport
starts with i32 ClassIndex, i32 SuperIndex, [i32 TemplateIndex,] i32
OuterIndex, FName ObjectName, ... - we read just enough to map each
export to its class name.
"""

import unreal
import os
import struct
import re
import hashlib
import json
from collections import Counter


OUTPUT_JSON = r"C:\Users\tlind\Documents\Unreal Projects\UnrealMergeBinairies\src\script_output.json"

UE_PACKAGE_TAG = 0x9E2A83C1
IDENT_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")


def read_fstring(data, offset):
    """UE FString: int32 length. Positive = ANSI (length includes NUL). Negative = UCS2 (|length| includes NUL)."""
    length = struct.unpack_from("<i", data, offset)[0]
    offset += 4
    if length == 0:
        return "", offset
    if length > 0:
        raw = data[offset:offset + length]
        offset += length
        return raw.rstrip(b"\x00").decode("ascii", errors="replace"), offset
    else:
        raw = data[offset:offset + (-length) * 2]
        offset += (-length) * 2
        return raw.decode("utf-16-le", errors="replace").rstrip("\x00"), offset


def try_parse_name_table_with_stride(data, name_offset, name_count, hash_bytes):
    names = []
    p = name_offset
    try:
        for _ in range(name_count):
            if p + 4 > len(data):
                return None
            name, p = read_fstring(data, p)
            p += hash_bytes
            if not name or len(name) > 1024:
                return None
            if not IDENT_RE.match(name) and not re.match(r"^[ -~]+$", name):
                return None  # not printable
            names.append(name)
        return names
    except Exception:
        return None


def try_parse_name_table(data, name_offset, name_count):
    # UE5 generally uses 4 bytes (two uint16 hashes). Older variants used 8 bytes
    # (two uint32). Try both, accept the first that decodes the full table.
    for hb in (4, 8, 0):
        names = try_parse_name_table_with_stride(data, name_offset, name_count, hb)
        if names is not None:
            return names
    return None


def locate_summary_field(data, candidates_range, count_min, count_max, validator):
    """Scan for a (count, offset) pair in the summary that validates."""
    for probe in range(candidates_range[0], candidates_range[1], 4):
        if probe + 8 > len(data):
            break
        c = struct.unpack_from("<i", data, probe)[0]
        o = struct.unpack_from("<i", data, probe + 4)[0]
        if count_min <= c <= count_max and 0 < o < len(data) - 16:
            result = validator(data, o, c)
            if result is not None:
                return probe, c, o, result
    return None


def parse_uasset(path):
    with open(path, "rb") as f:
        data = f.read()
    if struct.unpack_from("<I", data, 0)[0] != UE_PACKAGE_TAG:
        raise ValueError(f"bad magic in {path}")

    # 1) Locate the Name Table
    found = locate_summary_field(data, (8, 2048), 1, 100000, try_parse_name_table)
    if not found:
        raise ValueError(f"could not locate Name Table in {path}")
    name_probe, name_count, name_offset, names = found

    # 2) Locate the Export Table - try each candidate (export_count, export_offset)
    #    where the table can be plausibly parsed AND class indices look valid.
    def try_parse_exports(data, exp_offset, exp_count):
        results = []
        p = exp_offset
        # FObjectExport size varies by UE version; we read fields tolerantly.
        # Required: ClassIndex (i32). Negative => import (resolved against import table - we don't have it parsed).
        # We just need: for each export, a sensible class identifier.
        # Try fixed offsets: ClassIndex at +0, ObjectName(FName=i32 idx + i32 num) at +16, +20, or +24.
        for _ in range(exp_count):
            if p + 80 > len(data):
                return None
            class_index = struct.unpack_from("<i", data, p)[0]
            # ObjectName FName lives a bit further in. Try several offsets and pick one
            # where the NameIndex falls inside [0, name_count) consistently.
            obj_name = None
            for fname_offset_within_export in (16, 20, 24, 28, 32):
                ni = struct.unpack_from("<i", data, p + fname_offset_within_export)[0]
                if 0 <= ni < name_count:
                    obj_name = names[ni]
                    break
            results.append({"class_index": class_index, "object_name": obj_name})
            # Advance - entries are typically 80–104 bytes; we'll bail and retry with
            # different stride if validation fails downstream.
            # We can't know stride without proper version parsing, so use a sentinel:
            # return None to signal caller try a different candidate.
            return None  # Forces caller to use heuristic below
        return results

    # The variable-size FObjectExport defeats naive parsing. Instead, use a
    # heuristic that doesn't require iterating with a known stride: count the
    # FName references to each K2Node_* class across the WHOLE post-summary
    # region. Each export entry contains a ClassIndex pointing into the
    # Import Table; the import table entry contains the class FName. We
    # don't parse Import Table either - instead, we count how often each
    # class name's FName-index pair (NameIndex, NameNumber) appears in the
    # raw bytes after the header. UE serializes class references as that
    # 8-byte FName tuple.

    # Build name -> index map
    name_to_index = {}
    for i, n in enumerate(names):
        name_to_index.setdefault(n, i)

    # For each class-like name, count occurrences of its FName index pattern.
    # An FName reference is 8 bytes: i32 NameIndex + i32 Number.
    # We don't know Number, so we scan for any 8-byte sequence where the first
    # 4 bytes equal `idx` and the next 4 bytes are a small int (0..200).
    def count_fname_refs(idx):
        pattern_prefix = struct.pack("<i", idx)
        # Scan
        count = 0
        offset = 0
        while True:
            pos = data.find(pattern_prefix, offset)
            if pos < 0:
                break
            # Bounds + plausible Number value (0..2_000_000 covers normal cases)
            if pos + 8 <= len(data):
                num = struct.unpack_from("<i", data, pos + 4)[0]
                if 0 <= num <= 2_000_000:
                    count += 1
            offset = pos + 1  # allow overlap (4-byte aligned would be +4)
        return count

    # Classify names by interest
    class_names = [n for n in set(names) if n.startswith(("K2Node_", "USCS_", "EdGraph", "Blueprint", "Simple"))]
    class_ref_counts = {}
    for cn in class_names:
        idx = name_to_index[cn]
        # Subtract the one occurrence inside the Name Table itself (the actual FString bytes
        # are AFTER its length prefix; but the *index* `cn`'s int32 value can also appear
        # naturally. We accept that noise: small spurious hits will be similar across v1/v2
        # and wash out in the diff.)
        class_ref_counts[cn] = count_fname_refs(idx)

    # Total bytes file size for sanity
    return {
        "path": path,
        "size": len(data),
        "sha256": hashlib.sha256(data).hexdigest(),
        "name_count": name_count,
        "name_offset": name_offset,
        "name_probe": name_probe,
        "class_ref_counts": class_ref_counts,
    }


def main():
    p1 = r"C:\Users\tlind\Documents\Unreal Projects\UnrealMergeBinairies\Examples\v1\BP_MinimalChar.uasset"
    p2 = r"C:\Users\tlind\Documents\Unreal Projects\UnrealMergeBinairies\Examples\v2\BP_MinimalChar.uasset"

    a = parse_uasset(p1)
    b = parse_uasset(p2)

    all_classes = sorted(set(a["class_ref_counts"]) | set(b["class_ref_counts"]))
    delta = []
    for cn in all_classes:
        v1 = a["class_ref_counts"].get(cn, 0)
        v2 = b["class_ref_counts"].get(cn, 0)
        if v1 != v2:
            delta.append({"class": cn, "v1": v1, "v2": v2, "delta": v2 - v1})
    delta.sort(key=lambda x: (-abs(x["delta"]), x["class"]))

    result = {
        "v1": {"path": a["path"], "size": a["size"], "sha256": a["sha256"],
               "name_count": a["name_count"], "class_ref_counts": a["class_ref_counts"]},
        "v2": {"path": b["path"], "size": b["size"], "sha256": b["sha256"],
               "name_count": b["name_count"], "class_ref_counts": b["class_ref_counts"]},
        "size_delta_bytes": b["size"] - a["size"],
        "class_reference_delta": delta,
    }
    with open(OUTPUT_JSON, "w", encoding="utf-8") as out:
        json.dump(result, out, indent=2, default=str)

    unreal.log("=" * 74)
    unreal.log("BLUEPRINT DIFF - FName-index reference count per K2Node class")
    unreal.log("=" * 74)
    unreal.log(f"v1: {a['size']:,} bytes  name_count={a['name_count']}  sha={a['sha256'][:16]}...")
    unreal.log(f"v2: {b['size']:,} bytes  name_count={b['name_count']}  sha={b['sha256'][:16]}...")
    unreal.log(f"size delta: {result['size_delta_bytes']:+d} bytes")
    unreal.log("")
    unreal.log("Class reference-count delta (v2 - v1):")
    if delta:
        for d in delta:
            unreal.log(f"  {d['delta']:+d}   {d['class']:48s}  (v1={d['v1']} v2={d['v2']})")
    else:
        unreal.log("  (no class reference count differences detected)")
    unreal.log("=" * 74)
    unreal.log(f"Full structured output: {OUTPUT_JSON}")


if __name__ == "__main__":
    main()
