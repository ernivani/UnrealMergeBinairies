import re
import os
import difflib


def extract_all_interesting_sequences(file_path):
    with open(file_path, 'rb') as f:
        b = f.read()


    strs = re.findall(rb'[ -~]{4,}', b)
    strs = [s.decode('ascii') for s in strs]

    keywords = (
        'K2Node', 'UEdGraph', 'Blueprint', 'EdGraph', 'BeginPlay', 
        'Tick', 'Component', 'Variable', 'Pin', 'Node', 'HP'
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

v1_sequences = extract_all_interesting_sequences(p1)
v2_sequences = extract_all_interesting_sequences(p2)

diff = difflib.unified_diff(
    v1_sequences, v2_sequences,
    fromfile='v1_Base', 
    tofile='v2_With_Get_HP',
    lineterm=''
)

print("==================================================")
print(" ANALYSE DE LA STRUCTURE INTERNE DU BLUEPRINT")
print("==================================================")

has_changes = False
for line in diff:
    if line.startswith('+') or line.startswith('-'):
        # On ignore les entêtes du diff pour n'avoir que les changements
        if not line.startswith('+++') and not line.startswith('---'):
            print(f"  {line}")
            has_changes = True

if not has_changes:
    print("\n  [!] Aucun changement structurel visible via les chaînes brutes.")
    print("      Cela confirme qu'Unreal a juste écrit des octets binaires (GUID/Pos)")
    print("      que seule la réflexion C++ saura décoder !")