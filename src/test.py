import re
import os

paths = [r'C:\Users\tlind\Documents\Unreal Projects\UnrealMergeBinairies\Examples\v1\BP_MinimalChar.uasset',
r'C:\Users\tlind\Documents\Unreal Projects\UnrealMergeBinairies\Examples\v2\BP_MinimalChar.uasset']

extracted = {}

for p in paths:
    with open(p, 'rb') as f: b = f.read()
    # ASCII strings 4+ chars
    strs = re.findall(rb'[ -~]{4,}', b)
    strs = [s.decode('ascii') for s in strs]
    # Filter to interesting ones
    interesting = [s for s in strs if any(k in s for k in (
        'K2Node', 'UEdGraph', 'Blueprint', 'EdGraph', 'UbergraphPages', 'FunctionGraphs', 'MacroGraphs',
        'EventGraph', 'BeginPlay', 'Tick', 'SceneRoot', 'Component', 'Variable', '/Script/', '/Game/',
        'Pin', 'Node', 'BPGC', 'SCS_Node'))]
    extracted[p] = (len(b), len(strs), interesting)
    for p,(sz,nstrs,interesting) in extracted.items():
        print('===', os.path.basename(os.path.dirname(p)), 'size=',sz,'strings=',nstrs)
        # uniq preserving order, first 60
        seen=set(); uniq=[]
        for s in interesting:
            if s not in seen:
                seen.add(s); uniq.append(s)
        for s in uniq[:80]:
            print(' ', s)
        print('...(', len(uniq), 'unique interesting strings)')
print()