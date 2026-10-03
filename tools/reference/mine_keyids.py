"""Extract the BlackWidow's onboard key-id table (id -> KEY_* name) and any non-empty Hypershift
mappings from Synapse's logs."""
import glob, os, re, json

D = os.path.expandvars(r'%LOCALAPPDATA%\Razer\RazerAppEngine\User Data\Logs')
files = sorted(glob.glob(os.path.join(D, '*.log')), key=os.path.getmtime)
ids, hyper = {}, {}
entry = re.compile(r'"(\d+)":\{"id":(\d+),"idHex"')
for f in files:
    for line in open(f, encoding='utf-8', errors='replace'):
        if '"HypershiftMode"' not in line:
            continue
        for m in entry.finditer(line):
            start = line.find('{', m.start() + len(m.group(1)) + 2)
            depth = 0
            for k in range(start, min(len(line), start + 6000)):
                depth += line[k] == '{'
                depth -= line[k] == '}'
                if depth == 0:
                    try:
                        e = json.loads(line[start:k + 1])
                    except Exception:
                        break
                    ids[e['id']] = e.get('inputID')
                    hs = e.get('HypershiftMode', {})
                    if hs.get('fnIdEnum'):
                        hyper[e['id']] = (e.get('inputID'), hs['fnIdEnum'], hs.get('fnDataByte'))
                    break
print(f'{len(ids)} key ids')
for i in sorted(ids):
    print(f'  {i:3d} 0x{i:02X}  {ids[i]}')
print('non-empty Hypershift mappings:', hyper or 'none')
json.dump({str(k): v for k, v in sorted(ids.items())}, open('keyids.json', 'w'), indent=1)
