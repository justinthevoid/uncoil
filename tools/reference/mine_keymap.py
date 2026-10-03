"""Extract the onboard key-mapping (OBM) structure for the BlackWidow from Synapse logs:
the per-key entry shape, the mapping modes available (Normal / Fn / Hypershift ...), and any commands
that write mappings to the device."""
import glob, os, re, json, collections

D = os.path.expandvars(r'%LOCALAPPDATA%\Razer\RazerAppEngine\User Data\Logs')
files = sorted(glob.glob(os.path.join(D, '*.log')), key=os.path.getmtime)

entry_re = re.compile(r'"(\d+)":\{"id":\1,"idHex":"0x[0-9a-f]+",')
best = None
mode_keys = collections.Counter()
fn_enums = collections.Counter()
write_cmds = collections.Counter(); write_samples = {}
for f in files:
    for line in open(f, encoding='utf-8', errors='replace'):
        if '"idHex"' in line and 'NormalMode' in line:
            for m in re.finditer(r'"(NormalMode|FnMode|HypershiftMode|HyperShiftMode|Fn_Mode|[A-Za-z]+Mode)":\{"fnId"', line):
                mode_keys[m.group(1)] += 1
            for m in re.finditer(r'"fnIdEnum":"(\w+)"', line):
                fn_enums[m.group(1)] += 1
            if best is None:
                i = line.find('"124":{"id":124')
                if i >= 0:
                    # grab the balanced JSON object for key 124
                    depth, j = 0, line.find('{', i)
                    for k in range(j, len(line)):
                        depth += line[k] == '{'
                        depth -= line[k] == '}'
                        if depth == 0:
                            best = line[j:k + 1]
                            break
        for m in re.finditer(r'"commandDesc":"([^"]*(?:[Mm]apping|[Kk]ey|OBM|[Pp]rofile|[Ff]n)[^"]*)","dataSend":\[([\d,]{0,80})', line):
            write_cmds[m.group(1)] += 1
            write_samples.setdefault(m.group(1), m.group(2))
        for m in re.finditer(r'"command":\[(\d+),(\d+),(\d+)\],"commandDesc":"([^"]+)"', line):
            if re.search(r'[Mm]apping|[Kk]ey|OBM|[Pp]rofile|Fn', m.group(4)):
                write_cmds[f"{m.group(4)}  [size {m.group(1)} class 0x{int(m.group(2)):02X} id 0x{int(m.group(3)):02X}]"] += 1

print('== mapping modes per key:', dict(mode_keys))
print('== function types (fnIdEnum):', dict(fn_enums.most_common()))
print('== key 124 (0x7C) entry:')
if best:
    try:
        print(json.dumps(json.loads(best), indent=1)[:2500])
    except Exception:
        print(best[:2500])
print('== mapping/profile/key commands seen:')
for k, v in write_cmds.most_common(40):
    print(f'  {v:5d}  {k}   {write_samples.get(k, "")[:60]}')
