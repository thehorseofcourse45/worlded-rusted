import sys, collections
sys.argv=['x']
exec(open('blendtest.py').read().split('for v in sys.argv[1:]:')[0])
variants = ['road']
tile_dir = {}
for b in blends:
    if b['mainTile']=='darkgrass':
        for t in expand(b['blendTile']): tile_dir[t]=b['dir']
grass = expand('darkgrass')
table = collections.defaultdict(collections.Counter)
names8 = ['nw','n','ne','w','e','sw','s','se']
off = {'nw':(-1,-1),'n':(0,-1),'ne':(1,-1),'w':(-1,0),'e':(1,0),'sw':(-1,1),'s':(0,1),'se':(1,1)}
for v in variants:
    floor,multi = decode(f"{H}/exp/{v}/lots",900,900)
    h,w = floor.shape
    for y in range(2,h-2):
        pass
    isg = np.isin(floor, list(grass))
    street = np.char.startswith(floor.astype(str), 'blends_street')
    ys,xs = np.nonzero(street)
    for y,x in zip(ys,xs):
        if x<1 or y<1 or x>=w-1 or y>=h-1: continue
        pat = ''.join('G' if isg[y+dy,x+dx] else '-' for dx,dy in (off[n] for n in names8))
        if pat == '--------': continue
        st = multi.get((x,y), [])
        dirs = tuple(sorted(tile_dir[t] for t in st[1:] if t in tile_dir))
        table[pat][dirs]+=1
print(len(table),'patterns')
for pat,c in sorted(table.items(), key=lambda kv:-sum(kv[1].values()))[:22]:
    g=dict(zip(names8,pat))
    print(f"{pat[0:3]}/{pat[3]}.{pat[4]}/{pat[5:8]}  (nw n ne / w e / sw s se)  n={sum(c.values())}", dict(c.most_common(3)))
