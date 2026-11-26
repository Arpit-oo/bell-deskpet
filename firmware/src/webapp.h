#pragma once

// Control / settings app served at http://<pet>/  (single page, no build step).
static const char kWebApp[] PROGMEM = R"HTML(<!doctype html><html lang=en><head>
<meta charset=utf-8><meta name=viewport content="width=device-width,initial-scale=1,viewport-fit=cover">
<title>deskpet</title>
<link rel=preconnect href=https://fonts.googleapis.com>
<link href="https://fonts.googleapis.com/css2?family=Plus+Jakarta+Sans:wght@400;600;800&display=swap" rel=stylesheet>
<style>
:root{--bg:#07090c;--card:#0e1318;--line:#1b232c;--txt:#e9f3fa;--dim:#7d8b97;--acc:#7fe6ff;--acc2:#ffd65a}
*{box-sizing:border-box}html,body{margin:0;background:var(--bg);color:var(--txt);
font-family:"Plus Jakarta Sans",system-ui,sans-serif;-webkit-font-smoothing:antialiased}
body{background:radial-gradient(900px 500px at 80% -10%,color-mix(in srgb,var(--acc) 14%,transparent),transparent 60%),
radial-gradient(700px 400px at -10% 110%,color-mix(in srgb,var(--acc2) 10%,transparent),transparent 60%),var(--bg);min-height:100vh}
main{max-width:980px;margin:0 auto;padding:28px 20px 60px}
header{display:flex;align-items:center;justify-content:space-between;gap:16px;flex-wrap:wrap;margin-bottom:22px}
.logo{display:flex;align-items:center;gap:12px;font-weight:800;font-size:26px;letter-spacing:-.02em}
.eyes{display:flex;gap:6px}.eyes i{width:14px;height:18px;border-radius:6px;background:var(--acc);display:block;animation:bl 4s infinite}
@keyframes bl{0%,92%,100%{transform:scaleY(1)}95%{transform:scaleY(.1)}}
.pills{display:flex;gap:8px;flex-wrap:wrap}.pill{font-size:12px;padding:6px 10px;border-radius:999px;border:1px solid var(--line);color:var(--dim);background:#0b1015}
.pill b{color:var(--txt);font-weight:600}
.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(300px,1fr));gap:16px}
.card{background:color-mix(in srgb,var(--card) 88%,transparent);border:1px solid var(--line);border-radius:20px;padding:22px;backdrop-filter:blur(8px)}
.card h2{margin:0 0 16px;font-size:13px;letter-spacing:.14em;text-transform:uppercase;color:var(--dim);font-weight:600}
.chips{display:flex;flex-wrap:wrap;gap:8px}
button,.chip{font:inherit;font-size:14px;color:var(--txt);background:#121a21;border:1px solid var(--line);border-radius:12px;padding:10px 14px;cursor:pointer;transition:.15s}
button:hover{border-color:var(--acc);color:var(--acc)}button.primary{background:var(--acc);color:#04121a;border-color:var(--acc);font-weight:700}
button.primary:hover{filter:brightness(1.08);color:#04121a}
input[type=text],input[type=number]{font:inherit;color:var(--txt);background:#0a0f14;border:1px solid var(--line);border-radius:12px;padding:11px 13px;width:100%}
.row{display:flex;gap:10px;align-items:center}.row>*{flex:1}.row>button{flex:0 0 auto}
.time{font-size:54px;font-weight:800;letter-spacing:-.03em;font-variant-numeric:tabular-nums;margin:4px 0 2px}
.sub{color:var(--dim);font-size:13px}
.sw{display:flex;gap:10px;flex-wrap:wrap}.sw button{width:46px;height:46px;border-radius:14px;padding:0;position:relative}
.sw button.on{outline:2px solid var(--txt);outline-offset:3px}
.tog{display:flex;justify-content:space-between;align-items:center;padding:10px 0;border-top:1px solid var(--line)}
.tog:first-of-type{border-top:0}
.switch{width:44px;height:26px;border-radius:99px;background:#1b252e;position:relative;cursor:pointer;transition:.2s;flex:0 0 auto}
.switch:after{content:"";position:absolute;top:3px;left:3px;width:20px;height:20px;border-radius:50%;background:#cfd8df;transition:.2s}
.switch.on{background:var(--acc)}.switch.on:after{left:21px;background:#04121a}
.seg{display:flex;background:#0a0f14;border:1px solid var(--line);border-radius:12px;padding:3px}
.seg button{flex:1;border:0;background:transparent;border-radius:9px}.seg button.on{background:#1c2630;color:var(--acc)}
input[type=color]{width:46px;height:46px;border:1px solid var(--line);border-radius:14px;background:none;padding:3px;cursor:pointer}
.toast{position:fixed;left:50%;bottom:22px;transform:translateX(-50%) translateY(80px);background:#101820;border:1px solid var(--line);
padding:10px 16px;border-radius:12px;transition:.25s;font-size:14px}.toast.show{transform:translateX(-50%)}
</style></head><body><main>
<header><div class=logo><div class=eyes><i></i><i></i></div>deskpet</div>
<div class=pills><span class=pill>mood <b id=pm>-</b></span><span class=pill>companion <b id=pc>-</b></span>
<span class=pill>claude <b id=pl>-</b></span><span class=pill id=pmu style=display:none>&#9835; <b id=pmt></b></span></div></header>
<div class=grid>
<section class=card><h2>Pomodoro</h2><div class=time id=pt>25:00</div><div class=sub id=ps>ready when you are</div>
<div class=row style="margin-top:16px"><button class=primary onclick="pomo('start')">Start</button><button onclick="pomo('skip')">Skip</button><button onclick="pomo('stop')">Stop</button></div>
<div class=row style="margin-top:14px"><label class=sub>focus <input type=number id=pf min=1 max=90 onchange="set({pomo_focus:+this.value})"></label>
<label class=sub>break <input type=number id=pb min=1 max=30 onchange="set({pomo_break:+this.value})"></label></div></section>
<section class=card><h2>Say something</h2><div class=row><input type=text id=say maxlength=80 placeholder="LET'S GOOO"><button class=primary onclick=sayIt()>Send</button></div>
<h2 style="margin-top:22px">Hype level</h2><div class=seg id=hype><button data-v=0>chill</button><button data-v=1>normal</button><button data-v=2>max</button></div>
<h2 style="margin-top:22px">Name</h2><input type=text id=name maxlength=20 onchange="set({name:this.value})"></section>
<section class=card><h2>Mood</h2><div class=chips id=moods></div>
<h2 style="margin-top:22px">Animations</h2><div class=chips id=anims></div></section>
<section class=card><h2>Theme</h2><div class=sw id=themes></div>
<div class=row style="margin-top:16px"><span class=sub>custom eyes</span><input type=color id=ce onchange="set({eye:this.value})">
<span class=sub>accent</span><input type=color id=ca onchange="set({accent:this.value})"></div></section>
<section class=card><h2>Display</h2>
<div class=tog><span>Flip 180&deg;</span><div class=switch data-k=flip></div></div>
<div class=tog><span>Eyebrows</span><div class=switch data-k=brows></div></div>
<div class=tog><span>Idle animations</span><div class=switch data-k=quirks></div></div>
<div class=tog><span>Lyrics</span><div class=switch data-k=lyrics></div></div>
<div class=tog><span>Sing along</span><div class=switch data-k=sing></div></div>
<p class=sub style="margin:12px 0 0">The panel only rotates in 90&deg; steps, so flip = 180&deg;.</p></section>
</div></main><div class=toast id=toast></div>
<script>
const $=s=>document.querySelector(s);let S={};
const THEMES={ice:["#78e6ff","#ffd65a"],mint:["#78ffbe","#ffaa78"],sunset:["#ff965a","#ff5a8c"],sakura:["#ffa0d2","#be96ff"],matrix:["#50ff78","#50ff78"],mono:["#ffffff","#c8c8c8"]};
const MOODS=["idle","happy","excited","focus","music","sleepy","sus","sad","angry","surprised","love"];
const ANIMS={lookaround:"look around",curious:"curious",wiggle:"wiggle",yawn:"yawn",peek:"peek"};
function toast(t){const e=$('#toast');e.textContent=t;e.classList.add('show');clearTimeout(e._t);e._t=setTimeout(()=>e.classList.remove('show'),1400)}
async function post(u,d){const r=await fetch(u,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(d)});return r.json().catch(()=>({}))}
async function set(d){S=await post('/api/settings',d);paint();toast('saved')}
function pomo(a){post('/api/pomodoro',{action:a}).then(status)}
function sayIt(){const v=$('#say').value.trim();if(v){post('/api/say',{text:v,mood:'excited',ms:7000});$('#say').value='';toast('sent')}}
$('#say').addEventListener('keydown',e=>{if(e.key==='Enter')sayIt()});
MOODS.forEach(m=>{const b=document.createElement('button');b.textContent=m;b.onclick=()=>post('/api/mood',{mood:m,ms:6000}).then(status);$('#moods').appendChild(b)});
Object.entries(ANIMS).forEach(([k,v])=>{const b=document.createElement('button');b.textContent=v;b.onclick=()=>post('/api/anim',{name:k});$('#anims').appendChild(b)});
Object.entries(THEMES).forEach(([k,c])=>{const b=document.createElement('button');b.title=k;b.dataset.t=k;
b.style.background=`linear-gradient(135deg,${c[0]} 55%,${c[1]} 55%)`;b.onclick=()=>set({theme:k});$('#themes').appendChild(b)});
document.querySelectorAll('.switch').forEach(s=>s.onclick=()=>set({[s.dataset.k]:!S[s.dataset.k]}));
document.querySelectorAll('#hype button').forEach(b=>b.onclick=()=>set({hype:+b.dataset.v}));
function paint(){document.documentElement.style.setProperty('--acc',S.eye||'#7fe6ff');document.documentElement.style.setProperty('--acc2',S.accent||'#ffd65a');
document.querySelectorAll('.switch').forEach(s=>s.classList.toggle('on',!!S[s.dataset.k]));
document.querySelectorAll('#themes button').forEach(b=>b.classList.toggle('on',b.dataset.t===S.theme));
document.querySelectorAll('#hype button').forEach(b=>b.classList.toggle('on',+b.dataset.v===S.hype));
$('#ce').value=S.eye||'#78e6ff';$('#ca').value=S.accent||'#ffd65a';
if(document.activeElement!==$('#name'))$('#name').value=S.name||'';$('#pf').value=S.pomo_focus;$('#pb').value=S.pomo_break}
const mmss=s=>String(Math.floor(s/60)).padStart(2,'0')+':'+String(s%60).padStart(2,'0');
async function status(){try{const j=await (await fetch('/api/status')).json();$('#pm').textContent=j.mood;$('#pc').textContent=j.companion?'online':'offline';
$('#pl').textContent=j.claude;const p=j.pomodoro;if(p.phase==='off'){$('#pt').textContent=mmss((S.pomo_focus||25)*60);$('#ps').textContent='ready when you are'}
else{$('#pt').textContent=mmss(p.remaining_s);$('#ps').textContent=(p.phase==='focus'?'focus':'break')+' · round '+p.round}
$('#pmu').style.display=j.music.playing?'':'none';$('#pmt').textContent=j.music.track}catch(e){}}
fetch('/api/settings').then(r=>r.json()).then(j=>{S=j;paint();status()});setInterval(status,1000);
</script></body></html>)HTML";
