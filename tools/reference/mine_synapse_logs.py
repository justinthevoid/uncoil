"""Build a protocol catalog for the BlackWidow V4 Pro 75% (PIDs 691/692/693) from Synapse's own logs."""
import glob, os, re, json, collections
D = os.path.expandvars(r'%LOCALAPPDATA%\Razer\RazerAppEngine\User Data\Logs')
OUT = os.path.join(os.getcwd(), 'protocol-catalog.json')
files = sorted(glob.glob(os.path.join(D, '*.log')), key=os.path.getmtime)

cmd_re = re.compile(r'"command":\[(\d+),(\d+),(\d+)\],"commandDesc":"([^"]+)","dataSend":\[([\d,]+)\]')
res_re = re.compile(r'"status":"(\w+)","packetSizeReturned":(\d+),"data":\[([\d,]*)\]')
json_re = re.compile(r'"jsonData":(\{.*?\}),"command"')
ev_re = re.compile(r'"buffer":\[([\d,]+)\]')
evdesc_re = re.compile(r'"rawBuffer":\[([\d,]+)\].*?"recordIdDesc":"(\w+)","data":(\{.*?\}),"productId":(\d+)')

cmds = {}
events = collections.OrderedDict()
pid_of = lambda line: (re.search(r'(?:productId|Pid)[":]+(\d+)', line) or [None, '?'])[1]
for f in files:
    for line in open(f, encoding='utf-8', errors='replace'):
        if '"commandDesc"' in line:
            m = cmd_re.search(line)
            if m:
                size, cls, cid, desc, ds = m.groups()
                key = desc
                e = cmds.setdefault(key, {'class': int(cls), 'id': int(cid), 'size': int(size), 'examples': [], 'pids': set()})
                e['pids'].add(pid_of(line))
                args = [int(x) for x in ds.split(',')][8:8 + int(size)]
                r = res_re.search(line); j = json_re.search(line)
                ex = {'args': args, 'status': r.group(1) if r else None,
                      'reply': [int(x) for x in r.group(3).split(',') if x] if r else None,
                      'json': j.group(1)[:300] if j else None}
                if len(e['examples']) < 4 and ex['args'] not in [x['args'] for x in e['examples']]:
                    e['examples'].append(ex)
        if 'rawBuffer' in line and 'recordIdDesc' in line:
            m = evdesc_re.search(line)
            if m:
                buf, rec, data, pid = m.groups()
                b = [int(x) for x in buf.split(',')]
                key = (rec, tuple(b[:4]))
                if key not in events:
                    events[key] = {'record': rec, 'buffer': b[:8], 'data': data[:200], 'pid': pid}

for e in cmds.values():
    e['pids'] = sorted(e['pids'])
json.dump({'commands': cmds, 'events': list(events.values())}, open(OUT, 'w'), indent=1)

print(f'{len(cmds)} distinct commands, {len(events)} distinct input events -> {OUT}\n')
for desc, e in sorted(cmds.items(), key=lambda kv: (kv[1]['class'], kv[1]['id'])):
    ex = e['examples'][0]
    print(f"  0x{e['class']:02X}/0x{e['id']:02X} sz{e['size']:<3} {desc:48s} args={ex['args'][:10]} {('json=' + ex['json'][:90]) if ex['json'] and ex['json'] != '{}' else ''}")
print('\nINPUT EVENTS:')
for v in events.values():
    print(f"  [{v['record']}] {v['buffer']}  {v['data'][:110]}")
