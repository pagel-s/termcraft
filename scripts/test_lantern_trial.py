#!/usr/bin/env python3
"""Exercise the complete offline trial through the shipped CLI and agent tools."""
import subprocess,tempfile,os,json,sys,time,pathlib,argparse,atexit,shutil,hashlib
parser=argparse.ArgumentParser(description="Play Lantern Run through the real CLI/MCP under normal rules in disposable saves.")
parser.add_argument('--binary',type=pathlib.Path,default=pathlib.Path('termcraft'))
parser.add_argument('--report',type=pathlib.Path)
parser.add_argument('--keep-world',action='store_true',help='Retain only the temporary test world for terminal review')
args=parser.parse_args()
binary=str(args.binary.resolve())
root=pathlib.Path(tempfile.mkdtemp(prefix='termcraft-lantern-cli-'))
env=dict(os.environ,TERMCRAFT_SAVE_DIR=str(root));env.pop('NO_COLOR',None)
subprocess.run([binary,'trial','create','audit'],env=env,check=True,capture_output=True)
p=subprocess.Popen([binary,'mcp'],env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
seq=0;simulated_ticks=0
def cleanup():
 if p.poll() is None:p.kill();p.wait()
 if not args.keep_world:shutil.rmtree(root)
atexit.register(cleanup)

def call(name,args):
 global seq
 seq+=1;p.stdin.write(json.dumps({'jsonrpc':'2.0','id':seq,'method':'tools/call','params':{'name':name,'arguments':args}})+'\n');p.stdin.flush()
 r=json.loads(p.stdout.readline())['result']
 assert not r.get('isError'),r
 return r['structuredContent']
def act(n=1,**kw):
 global simulated_ticks
 simulated_ticks+=n
 return call('act',dict(ticks=n,**kw))
def show(s,label):print(label,json.dumps({k:s[k] for k in ['player','objective','entities']}),flush=True)
s=call('enter_world',{'id':'audit'});show(s,'spawn')
s=act(40,direction='right');show(s,'course')
s=act(9,direction='right');show(s,'edge')
s=act(30,direction='right',jump=True);show(s,'jump')
# Advance to the ruins, reacting to normal actor observations.
stalls=0;last_x=s['player']['x']
for i in range(220):
 s=call('observe',{'radius':12})
 entities=[e for e in s['entities'] if e['kind'] in ('zombie','skeleton') and e['health']>0]
 if abs(s['player']['x']-last_x)<.01:stalls+=1
 else:stalls=0
 last_x=s['player']['x']
 if stalls>=3 and 30<s['player']['x']<40 and (not entities or min(abs(e['x']-s['player']['x']) for e in entities)>2.4):
  cover=31 if s['player']['x']<35 else 39
  call('select_slot',{'slot':2});s=act(40,primary=True,target={'x':cover,'y':62});s=act(16,direction='right',jump=True);call('select_slot',{'slot':0});stalls=0
  continue
 if entities:
  e=min(entities,key=lambda e:abs(e['x']-s['player']['x']))
  dx=e['x']-s['player']['x'];target={'x':int(e['x']//1),'y':int((e['y']-.8)//1)}
  kw={'primary':True,'target':target} if abs(dx)<11 else {}
  if abs(dx)>1.55:kw['direction']='right' if dx>0 else 'left'
  # Jump over cover only when normal progress stalls.
  if i%6==5:kw['jump']=True
  s=act(4,**kw)
 elif s['player']['y']>64.1:s=act(16,direction='right',jump=True)
 else:
  if abs(s['player']['x']-last_x)<.01:stalls+=1
  else:stalls=0
  if stalls>=3 and 30<s['player']['x']<40:
   cover=31 if s['player']['x']<35 else 39
   call('select_slot',{'slot':2});s=act(40,primary=True,target={'x':cover,'y':62});s=act(16,direction='right',jump=True);call('select_slot',{'slot':0});stalls=0
  else:
   last_x=s['player']['x'];s=act(4,direction='right',jump=(i%8==7))
 if i%10==0:show(s,'fight'+str(i))
 if s['trial']['stage']=='build':break
 if s['ui']['dead']:raise AssertionError('CLI player died in trial')
assert s['trial']['stage']=='build',s['trial'];show(s,'guards cleared')
# Walk/jump to chest, then use the same UI quick-move operations humans use.
for i in range(100):
 x=s['player']['x']
 if 43.3<x<44.1:s=call('use',{'x':44,'y':63})
 if abs(x-45.5)<.7:break
 s=act(2,direction='right' if x<45.5 else 'left',jump=i%6==5)
s=call('use',{'x':46,'y':63});assert s['ui']['chest'],s
for slot in range(4):s=call('inventory_click',{'slot':slot,'kind':2})
s=call('close_inventory',{});show(s,'supplies')
# Reach the abutment, jumping the ordinary full-block supply chest.
for i in range(80):
 x=s['player']['x']
 if x>=49.2:break
 s=act(4,direction='right',jump=45.4<x<47.3)
assert 49.1<=s['player']['x']<50.4,s['player']
slot=next(i for i,e in enumerate(s['inventory']) if e and e['item_type']=='Planks')
s=call('select_slot',{'slot':slot})
show(s,'abutment approach')
s=act(5,direction='right',jump=True);s=act(8)
show(s,'abutment top')
for x in [52,53]:
 before=sum(e['count'] for e in s['inventory'] if e and e['item_type']=='Planks')
 s=call('use',{'x':x,'y':63})
 after=sum(e['count'] for e in s['inventory'] if e and e['item_type']=='Planks')
 assert after==before-1,('initial placement failed',x,s['player'])
for x in range(54,60):
 show(s,'placing '+str(x))
 before=sum(e['count'] for e in s['inventory'] if e and e['item_type']=='Planks')
 s=call('use',{'x':x,'y':63})
 after=sum(e['count'] for e in s['inventory'] if e and e['item_type']=='Planks')
 assert after==before-1,('bridge placement failed',x,s['player'])
 for _ in range(16):
  if s['player']['x']>=x-1.0:break
  s=act(1,direction='right')
show(s,'bridge')
for _ in range(50):
 if s['player']['x']>=62.4:break
 s=act(1,direction='right')
slot=next(i for i,e in enumerate(s['inventory']) if e and e['item_type']=='Torch')
s=call('select_slot',{'slot':slot});s=call('use',{'x':65,'y':60});s=act(2)
show(s,'signal')
assert s['trial']['stage']=='complete',s['trial']
call('leave_world',{});s=call('enter_world',{'id':'audit'});assert s['trial']['stage']=='complete'
call('leave_world',{});p.stdin.close();p.wait(timeout=8)
report={'status':'passed','binary_sha256':hashlib.sha256(pathlib.Path(binary).read_bytes()).hexdigest(),'calls':seq,'simulated_ticks':simulated_ticks,'stage':s['trial']['stage'],'player':s['player'],'normal_inventory_and_placement_rules':True,'save_reopen':True,'limits':'Deterministic scripted actions with known map coordinates through real stdio MCP. No teleport/loadout or state edits. Excludes human enjoyment and emulator/WAN latency.'}
if args.keep_world:report['world_root']=str(root)
if args.report:args.report.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report))
