import re
import os

def extract_interesting_strings(file_path):
    with open(file_path, 'rb') as f:
        b = f.read()


    strs = re.findall(rb'[ -~]{4,}', b)
    strs = [s.decode('ascii') for s in strs]

    keywords = (
        'K2Node', 'UEdGraph', 'Blueprint', 'EdGraph', 'UbergraphPages', 'FunctionGraphs', 'MacroGraphs',
        'EventGraph', 'BeginPlay', 'Tick', 'SceneRoot', 'Component', 'Variable', '/Script/', '/Game/',
        'Pin', 'Node', 'BPGC', 'SCS_Node'
    )

    seen = set()
    uniq = []

    for s in strs:
        if s not in seen and any(k in s for k in keywords):
            seen.add(s)
            uniq.append(s)

    return uniq

p1 = r'C:\Users\tlind\Documents\Unreal Projects\UnrealMergeBinairies\Examples\v1\BP_MinimalChar.uasset'
p2 = r'C:\Users\tlind\Documents\Unreal Projects\UnrealMergeBinairies\Examples\v2\BP_MinimalChar.uasset'

v1_strings = extract_interesting_strings(p1)
v2_strings = extract_interesting_strings(p2)

set_v1 = set(v1_strings)
set_v2 = set(v2_strings)

removed = [s for s in v1_strings if s not in set_v2]
added = [s for s in v2_strings if s not in set_v1]
unchanged = [s for s in v2_strings if s in set_v1]

print("==================================================")
print(f" DIFFING ASSET STRINGS: {os.path.basename(p1)}")
print("==================================================")

print(f"\n[-] REMOVED IN V2 ({len(removed)} strings):")

if removed:
    for s in removed[:40]:  # Cap at 40 to avoid scrolling spam
        print(f"  {s}")
    if len(removed) > 40: print(f"  ... and {len(removed) - 40} more")
else:
    print("  (None)")

print(f"\n[+] ADDED IN V2 ({len(added)} strings):")
if added:
    for s in added[:40]:
        print(f"  {s}")
    if len(added) > 40: print(f"  ... and {len(added) - 40} more")
else:
    print("  (None)")

print(f"\n[=] SUMMARY:")
print(f"  Total interesting strings in V1: {len(v1_strings)}")
print(f"  Total interesting strings in V2: {len(v2_strings)}")
print(f"  Unchanged tokens shared by both: {len(unchanged)}")