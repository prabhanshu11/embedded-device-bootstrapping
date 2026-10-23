#!/usr/bin/env python3
"""pihole-ui — a touch-first web page ON the Pi 5 showing every Pi-hole switch and the blocked trail.

His words 2026-10-11: "CReate a ui around all the pi-hole business, so that I am able to access it
and see what switches and knobs are ON, and also trace what all requests have been blocked." and
"except porn, everything else, I should be able to ask an agent to override".

What it can change, and only through the two bounded tools that already exist on the Pi:
  * `sudo -n pihole-pause GROUP MINUTES REASON` / `pihole-pause resume GROUP`
    (four groups only, max 240 min, reason required, Telegram-notified, auto re-enabled)
  * `sudo -n pihole-unlock request|cancel` (the 6-hour admin-password release to Telegram)
What it can NOT do, by design: log in to the Pi-hole admin, show the admin password, switch global
blocking off, touch the Default group (porn + ads lists) or any list/rule.

Reads go through /usr/local/bin/pihole-api (FTL cli_pw; user pi in group pihole).
Stdlib only (python 3.13, Debian 13). Listens on 0.0.0.0:8780 (LAN). Installed by
bootstrap/rpi5/setup-pihole-ui.sh as /usr/local/bin/pihole-ui + pihole-ui.service.

Env (for local testing with stubs): PIHOLE_UI_BIN (dir of the tools, default /usr/local/bin),
PIHOLE_UI_SUDO (prefix for the root tools, default "sudo -n"; set "" to call them directly),
PIHOLE_UI_PORT (default 8780).
"""
import json
import os
import re
import shlex
import subprocess
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

BIN = os.environ.get("PIHOLE_UI_BIN", "/usr/local/bin")
SUDO = shlex.split(os.environ.get("PIHOLE_UI_SUDO", "sudo -n"))
PORT = int(os.environ.get("PIHOLE_UI_PORT", "8780"))
TIMEOUT = 20
MAX_PAUSE_MIN = 240

PAUSABLE = ("always-shop", "always-video-phone", "bedtime-feeds", "bedtime-full")
DESCRIPTIONS = {
    "Default": ("Porn + ads lists (always on)",
                "The 4 porn blocklists + StevenBlack ads, with CleanBrowsing Adult as the upstream DNS. "
                "Applies to every device. Never pausable from here or anywhere."),
    "always-shop": ("Amazon shopping",
                    "Amazon shopping domains (8 regexes). Network-wide via the 192.168.0.0/24 client; infrastructure and the work laptop exempt. Always on."),
    "always-video-phone": ("YouTube (every device except the laptop)",
                           "All of YouTube + the DNS-over-HTTPS escape hatches. Network-wide via the 192.168.0.0/24 "
                           "client; the laptop keeps its own entry (Shorts-only via Chrome policy), infrastructure and "
                           "the work laptop are exempt. Always on."),
    "bedtime-feeds": ("Bedtime: feeds",
                      "Netflix, YouTube, Instagram, X, Reddit, TikTok, streaming + DoH. 00:00-06:00 on laptop + "
                      "phone (switched on and off by the bedtime engine on the laptop)."),
    "bedtime-full": ("Bedtime: everything",
                     "Everything except an allowlist (Anthropic, GitHub, PyPI, Telegram ...). 00:00-06:00 on the "
                     "laptop (switched by the bedtime engine)."),
}
ALSO_ON = [
    "Laptop Chrome policy blocks youtube.com/shorts and Amazon inside the browser (static, not a Pi-hole switch).",
    "Upstream DNS is CleanBrowsing Adult (185.228.168.10 / 185.228.169.11): it blocks adult sites a second time.",
]
BLOCKED_STATUSES = ("GRAVITY", "REGEX", "DENYLIST", "GRAVITY_CNAME", "REGEX_CNAME", "DENYLIST_CNAME",
                    "EXTERNAL_BLOCKED_IP", "EXTERNAL_BLOCKED_NULL", "EXTERNAL_BLOCKED_NXRA",
                    "EXTERNAL_BLOCKED_EDE15", "SPECIAL_DOMAIN")
STATUS_WORDS = {
    "GRAVITY": "blocklist", "GRAVITY_CNAME": "blocklist (via CNAME)",
    "REGEX": "regex rule", "REGEX_CNAME": "regex rule (via CNAME)",
    "DENYLIST": "exact deny rule", "DENYLIST_CNAME": "exact deny rule (via CNAME)",
    "EXTERNAL_BLOCKED_IP": "upstream (CleanBrowsing)", "EXTERNAL_BLOCKED_NULL": "upstream (CleanBrowsing)",
    "EXTERNAL_BLOCKED_NXRA": "upstream (CleanBrowsing)", "EXTERNAL_BLOCKED_EDE15": "upstream (CleanBrowsing)",
    "SPECIAL_DOMAIN": "special domain (e.g. Mozilla DoH canary)",
}


class ToolError(Exception):
    pass


def run(argv, root=False):
    """Run one tool with an explicit argv (never a shell); return stdout or raise ToolError."""
    cmd = (SUDO if root else []) + argv
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=TIMEOUT)
    except subprocess.TimeoutExpired:
        raise ToolError(f"{os.path.basename(argv[0])} timed out after {TIMEOUT}s")
    except OSError as e:
        raise ToolError(f"{os.path.basename(argv[0])}: {e.strerror}")
    if r.returncode != 0:
        msg = (r.stderr.strip() or r.stdout.strip() or f"exit {r.returncode}").splitlines()[-1]
        raise ToolError(f"{os.path.basename(argv[0])}: {msg[:300]}")
    return r.stdout


def api(path):
    """GET one Pi-hole API path through pihole-api (read only)."""
    out = run([os.path.join(BIN, "pihole-api"), "GET", path])
    try:
        return json.loads(out or "{}")
    except ValueError:
        raise ToolError(f"pihole-api {path}: not JSON")


def pause_tool(*args):
    return run([os.path.join(BIN, "pihole-pause"), *args], root=True)


def unlock_tool(*args):
    return run([os.path.join(BIN, "pihole-unlock"), *args], root=True)


_cache = {}
_cache_lock = threading.Lock()


def cached(key, ttl, fn):
    now = time.time()
    with _cache_lock:
        hit = _cache.get(key)
        if hit and now - hit[0] < ttl:
            return hit[1]
    val = fn()
    with _cache_lock:
        _cache[key] = (now, val)
    return val


def rules():
    """id -> rule dict for regex/exact domain rules, id -> adlist, gid -> group name (cached 60 s)."""
    def load():
        doms = {d["id"]: d for d in api("/api/domains").get("domains", [])}
        lists = {l["id"]: l for l in api("/api/lists").get("lists", [])}
        gnames = {g["id"]: g["name"] for g in api("/api/groups").get("groups", [])}
        return doms, lists, gnames
    return cached("rules", 60, load)


def devices():
    """ip -> name and mac -> [ips] from the network table (cached 60 s)."""
    def load():
        ip_name, mac_ips = {}, {}
        for d in api("/api/network/devices?max_devices=50&max_addresses=10").get("devices", []):
            for a in d.get("ips", []):
                if a.get("name"):
                    ip_name.setdefault(a["ip"], a["name"])
                mac_ips.setdefault(d.get("hwaddr", "").lower(), []).append(a["ip"])
        return ip_name, mac_ips
    return cached("devices", 60, load)


def parse_unlock(text):
    lines = [l for l in text.splitlines() if l.strip()]
    pending, requested, releases = False, None, None
    if lines and lines[0].startswith("pending:"):
        pending = True
        m = re.search(r"requested (\S+), releases (\S+)", lines[0])
        if m:
            requested, releases = m.group(1), m.group(2)
    return {"pending": pending, "requested": requested, "releases": releases, "log": lines[1:]}


def state():
    errors = []

    def safe(name, fn, default):
        try:
            return fn()
        except ToolError as e:
            errors.append(f"{name}: {e}")
            return default

    with ThreadPoolExecutor(max_workers=5) as ex:
        f_block = ex.submit(safe, "blocking", lambda: api("/api/dns/blocking"), {})
        f_groups = ex.submit(safe, "groups", lambda: api("/api/groups").get("groups", []), [])
        f_clients = ex.submit(safe, "clients", lambda: api("/api/clients").get("clients", []), [])
        f_pause = ex.submit(safe, "pauses", lambda: json.loads(pause_tool("status") or "{}"), {})
        f_lock = ex.submit(safe, "lock", lambda: parse_unlock(unlock_tool("status")), None)
        f_dev = ex.submit(safe, "devices", devices, ({}, {}))
    blocking, groups, clients, pauses = f_block.result(), f_groups.result(), f_clients.result(), f_pause.result()
    lock, (ip_name, mac_ips) = f_lock.result(), f_dev.result()

    gname = {g["id"]: g["name"] for g in groups}
    out_groups = []
    for g in sorted(groups, key=lambda g: g["id"]):
        title, desc = DESCRIPTIONS.get(g["name"], (g["name"], g.get("comment") or ""))
        p = pauses.get(g["name"])
        out_groups.append({
            "name": g["name"], "id": g["id"], "enabled": bool(g.get("enabled")), "title": title,
            "description": desc, "pausable": g["name"] in PAUSABLE and g["id"] != 0,
            "pause": {"until": p.get("until"), "reason": p.get("reason"), "by": p.get("by"),
                      "since": p.get("since")} if p else None,
        })
    out_clients = []
    for c in clients:
        key = c.get("client", "")
        ips = mac_ips.get(key.lower(), []) if ":" in key and "." not in key else [key]
        out_clients.append({
            "client": key, "name": c.get("name") or "", "comment": c.get("comment") or "", "ips": ips,
            "groups": [gname.get(i, f"#{i}") for i in c.get("groups", [])],
        })
    seen = sorted(ip_name.items(), key=lambda kv: tuple(int(x) if x.isdigit() else 0 for x in kv[0].split(".")))
    return {
        "blocking": blocking.get("blocking", "unknown"), "blocking_timer": blocking.get("timer"),
        "groups": out_groups, "clients": out_clients, "lock": lock,
        "known_ips": [{"ip": ip, "name": n} for ip, n in seen if ":" not in ip and not ip.startswith("127.")],
        "also_on": ALSO_ON, "errors": errors, "now": datetime.now(timezone.utc).isoformat(timespec="seconds"),
    }


def blocked(client="", minutes=1440, limit=200, domain=""):
    until = int(time.time())
    since = until - minutes * 60
    doms, lists, gnames = rules()
    base = f"/api/queries?from={since}&until={until}&length={limit}"
    if client:
        base += f"&client_ip={client}"
    if domain:
        base += f"&domain=*{domain}*"

    def one(st):
        return api(f"{base}&status={st}").get("queries", [])

    rows, errors, total = [], [], 0
    with ThreadPoolExecutor(max_workers=4) as ex:
        futs = {st: ex.submit(one, st) for st in BLOCKED_STATUSES}
        for st, f in futs.items():
            try:
                rows.extend(f.result())
            except ToolError as e:
                errors.append(f"{st}: {e}")
    rows.sort(key=lambda q: q.get("time", 0), reverse=True)
    total = len(rows)
    out = []
    for q in rows[:limit]:
        st = q.get("status", "")
        lid = q.get("list_id")
        rule, groups = None, []
        if st.startswith(("REGEX", "DENYLIST")) and isinstance(lid, int) and lid in doms:
            d = doms[lid]
            rule = d.get("domain")
            groups = [gnames.get(i, f"#{i}") for i in d.get("groups", [])]
        elif st.startswith("GRAVITY"):
            if isinstance(lid, int) and lid in lists:
                rule = lists[lid].get("comment") or lists[lid].get("address")
            groups = ["Default"]
        cl = q.get("client") or {}
        out.append({
            "time": q.get("time"), "domain": q.get("domain"), "type": q.get("type"),
            "client_ip": cl.get("ip"), "client_name": cl.get("name"),
            "status": st, "why": STATUS_WORDS.get(st, st.lower()), "rule": rule, "groups": groups,
            "cname": q.get("cname"),
        })
    return {"rows": out, "count": len(out), "matched": total, "minutes": minutes, "errors": errors}


def why(domain):
    """Which adlists / rules contain this exact domain (read-only /api/search)."""
    r = api(f"/api/search/{domain}?partial=false&N=20").get("search", {})
    hits = [{"kind": "list", "name": g.get("comment") or g.get("address"), "address": g.get("address"),
             "type": g.get("type")} for g in r.get("gravity", [])]
    hits += [{"kind": d.get("kind"), "name": d.get("domain"), "type": d.get("type")} for d in r.get("domains", [])]
    return {"domain": domain, "hits": hits}


DOMAIN_RE = re.compile(r"^[A-Za-z0-9._-]{1,253}$")
IP_RE = re.compile(r"^[0-9a-fA-F:.]{2,45}$")


def clean_reason(s):
    s = re.sub(r"[\x00-\x1f\x7f]+", " ", str(s or "")).strip()
    return s[:200]


class Handler(BaseHTTPRequestHandler):
    server_version = "pihole-ui/1"

    def log_message(self, fmt, *args):  # one line per request to the journal
        sys.stderr.write(f"{self.client_address[0]} {fmt % args}\n")

    def send_json(self, obj, code=200):
        body = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Cache-Control", "no-store")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        u = urlparse(self.path)
        q = {k: v[-1] for k, v in parse_qs(u.query).items()}
        try:
            if u.path == "/":
                body = PAGE.encode()
                self.send_response(200)
                self.send_header("Content-Type", "text/html; charset=utf-8")
                self.send_header("Cache-Control", "no-store")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
            elif u.path == "/api/state":
                self.send_json(state())
            elif u.path == "/api/blocked":
                client = q.get("client", "")
                if client and not IP_RE.match(client):
                    return self.send_json({"error": "bad client ip"}, 400)
                dom = q.get("domain", "")
                if dom and not DOMAIN_RE.match(dom):
                    return self.send_json({"error": "bad domain filter"}, 400)
                minutes = max(1, min(int(q.get("minutes") or 1440), 7 * 1440))
                limit = max(1, min(int(q.get("limit") or 200), 1000))
                self.send_json(blocked(client, minutes, limit, dom))
            elif u.path == "/api/why":
                dom = q.get("domain", "")
                if not DOMAIN_RE.match(dom):
                    return self.send_json({"error": "bad domain"}, 400)
                self.send_json(why(dom))
            elif u.path == "/healthz":
                self.send_json({"ok": True})
            else:
                self.send_json({"error": "not found"}, 404)
        except ToolError as e:
            self.send_json({"error": str(e)}, 500)
        except ValueError:
            self.send_json({"error": "bad number in query"}, 400)
        except Exception as e:  # noqa: BLE001 — never a traceback to the page
            sys.stderr.write(f"GET {u.path} failed: {e!r}\n")
            self.send_json({"error": f"internal error: {type(e).__name__}"}, 500)

    def do_POST(self):
        u = urlparse(self.path)
        # JSON only: a plain cross-site <form> cannot post here without a CORS preflight.
        if not (self.headers.get("Content-Type") or "").startswith("application/json"):
            return self.send_json({"error": "Content-Type must be application/json"}, 415)
        try:
            n = int(self.headers.get("Content-Length") or 0)
            if n > 4096:
                return self.send_json({"error": "body too large"}, 413)
            data = json.loads(self.rfile.read(n) or b"{}")
            if not isinstance(data, dict):
                raise ValueError
        except ValueError:
            return self.send_json({"error": "body must be a JSON object"}, 400)
        try:
            if u.path == "/api/pause":
                group = data.get("group")
                if group not in PAUSABLE:
                    return self.send_json({"error": f"{group!r} is not pausable (porn/ads lists never are)"}, 400)
                try:
                    minutes = int(data.get("minutes"))
                except (TypeError, ValueError):
                    return self.send_json({"error": "minutes must be a whole number"}, 400)
                if not 1 <= minutes <= MAX_PAUSE_MIN:
                    return self.send_json({"error": f"minutes must be 1..{MAX_PAUSE_MIN}"}, 400)
                reason = clean_reason(data.get("reason"))
                if len(reason) < 3:
                    return self.send_json({"error": "a reason is required (it goes to the log and Telegram)"}, 400)
                reason = f"{reason} [pihole-ui from {self.client_address[0]}]"
                out = pause_tool(group, str(minutes), reason)
                self.send_json({"ok": True, "result": json.loads(out or "{}")})
            elif u.path == "/api/resume":
                group = data.get("group")
                if group not in PAUSABLE:
                    return self.send_json({"error": f"{group!r} is not ours to resume"}, 400)
                out = pause_tool("resume", group)
                self.send_json({"ok": True, "result": json.loads(out or "{}")})
            elif u.path == "/api/unlock":
                action = data.get("action")
                if action not in ("request", "cancel"):
                    return self.send_json({"error": "action must be request or cancel"}, 400)
                out = unlock_tool(action)
                self.send_json({"ok": True, "message": out.strip().splitlines()[:4]})
            else:
                self.send_json({"error": "not found"}, 404)
        except ToolError as e:
            self.send_json({"error": str(e)}, 500)
        except Exception as e:  # noqa: BLE001
            sys.stderr.write(f"POST {u.path} failed: {e!r}\n")
            self.send_json({"error": f"internal error: {type(e).__name__}"}, 500)


PAGE = r"""<!doctype html>
<html lang="en"><head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<meta name="theme-color" content="#0d1117">
<title>Pi-hole switches</title>
<style>
:root{--bg:#0d1117;--card:#161b22;--card2:#1c2330;--line:#30363d;--fg:#e6edf3;--dim:#8b949e;
 --on:#3fb950;--off:#8b949e;--pause:#d29922;--bad:#f85149;--acc:#58a6ff;}
*{box-sizing:border-box}
html,body{margin:0;background:var(--bg);color:var(--fg);font:16px/1.45 system-ui,-apple-system,"Segoe UI",Roboto,sans-serif}
body{padding:12px max(12px,env(safe-area-inset-right)) 80px max(12px,env(safe-area-inset-left))}
h1{font-size:1.35rem;margin:4px 0 12px;display:flex;gap:10px;align-items:center;flex-wrap:wrap}
h1 small{font-size:.8rem;color:var(--dim);font-weight:400}
h2{font-size:1.1rem;margin:26px 0 10px;border-bottom:1px solid var(--line);padding-bottom:6px}
.chips{display:flex;gap:10px;flex-wrap:wrap;margin-bottom:6px}
.chip{display:flex;align-items:center;gap:10px;min-height:56px;padding:8px 16px;border-radius:14px;
 background:var(--card);border:2px solid var(--line);font-weight:600;font-size:1.05rem}
.chip .ico{font-size:1.5rem;line-height:1}
.chip.on{border-color:var(--on)} .chip.off{border-color:var(--bad);border-style:dashed}
.chip.pend{border-color:var(--pause);border-style:dotted}
.chip small{display:block;font-weight:400;color:var(--dim);font-size:.8rem}
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(310px,1fr));gap:12px}
.card{background:var(--card);border:2px solid var(--line);border-radius:16px;padding:14px;display:flex;flex-direction:column;gap:8px}
.card.st-on{border-color:var(--on)} .card.st-off{border-style:dashed} .card.st-paused{border-color:var(--pause);border-style:double;border-width:4px}
.card .top{display:flex;justify-content:space-between;gap:8px;align-items:flex-start}
.card .title{font-weight:700;font-size:1.1rem}
.card .gname{font:12px ui-monospace,monospace;color:var(--dim)}
.state{display:flex;align-items:center;gap:6px;font-weight:800;letter-spacing:.04em;padding:4px 10px;border-radius:10px;background:var(--card2);white-space:nowrap}
.state .ico{font-size:1.25rem}
.st-on .state{color:var(--on)} .st-off .state{color:var(--off)} .st-paused .state{color:var(--pause)}
.desc{color:#c9d1d9;font-size:.95rem}
.pausebox{background:#2a2111;border:1px solid var(--pause);border-radius:10px;padding:8px 10px;font-size:.95rem}
.pausebox b{font-variant-numeric:tabular-nums;font-size:1.15rem}
.btns{display:flex;flex-wrap:wrap;gap:8px;margin-top:auto}
button,select,input,textarea{font:inherit;color:var(--fg)}
button{min-height:48px;min-width:48px;padding:8px 16px;border-radius:12px;border:2px solid var(--line);background:var(--card2);cursor:pointer;font-weight:600}
button:active{transform:scale(.97)}
button.primary{border-color:var(--acc);background:#132339}
button.warn{border-color:var(--pause);background:#2a2111}
button.danger{border-color:var(--bad);background:#2d1214}
button:disabled{opacity:.45;cursor:not-allowed}
.form{display:none;flex-direction:column;gap:8px;background:var(--card2);border-radius:12px;padding:10px;border:1px solid var(--line)}
.form.open{display:flex}
.form label{font-size:.9rem;color:var(--dim)}
input,select,textarea{min-height:48px;padding:10px 12px;border-radius:10px;border:2px solid var(--line);background:#0b0f14;width:100%}
textarea{min-height:72px;resize:vertical}
input:focus,select:focus,textarea:focus{outline:none;border-color:var(--acc)}
.row{display:flex;gap:8px;flex-wrap:wrap;align-items:center}
.row>*{flex:1 1 140px}
.lock{display:flex;flex-direction:column;gap:8px}
.log{font:12px/1.4 ui-monospace,monospace;color:var(--dim);white-space:pre-wrap;word-break:break-word}
.clients{display:flex;flex-direction:column;gap:8px}
.client{background:var(--card);border:1px solid var(--line);border-radius:12px;padding:10px 12px}
.tag{display:inline-block;padding:2px 8px;margin:2px 4px 2px 0;border-radius:8px;background:var(--card2);border:1px solid var(--line);font-size:.85rem}
.tag.dis{text-decoration:line-through;color:var(--dim)}
.filters{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:8px;margin-bottom:10px}
.seg{display:flex;gap:0;border-radius:12px;overflow:hidden;border:2px solid var(--line)}
.seg button{border:0;border-radius:0;flex:1;background:var(--card)}
.seg button.sel{background:#132339;color:#fff;box-shadow:inset 0 -4px 0 var(--acc)}
.seg button.sel::before{content:"\25CF  "}
.count{color:var(--dim);margin:6px 0 10px}
.count b{color:var(--fg);font-size:1.15rem}
table{width:100%;border-collapse:collapse}
th,td{text-align:left;padding:8px 6px;border-bottom:1px solid var(--line);vertical-align:top}
th{color:var(--dim);font-weight:600;font-size:.85rem;position:sticky;top:0;background:var(--bg)}
td.t{white-space:nowrap;font-variant-numeric:tabular-nums;color:var(--dim)}
td.d{word-break:break-all;font-weight:600}
.why{font-size:.88rem}
.why code{font-size:.8rem;background:var(--card2);padding:1px 5px;border-radius:5px;word-break:break-all}
.whyout{font-size:.85rem;color:#c9d1d9;margin-top:4px}
.linkbtn{min-height:36px;padding:2px 10px;font-size:.85rem;margin-top:4px}
@media (max-width:640px){
 table,thead,tbody,tr,td{display:block} thead{display:none}
 tr{background:var(--card);border:1px solid var(--line);border-radius:12px;margin-bottom:8px;padding:6px 8px}
 td{border:0;padding:3px 2px} td.t{font-size:.85rem}
}
.err{background:#2d1214;border:2px dashed var(--bad);border-radius:12px;padding:10px;margin:8px 0;display:none}
.err.show{display:block}
.toast{position:fixed;left:50%;bottom:16px;transform:translateX(-50%);background:var(--card2);border:2px solid var(--acc);
 border-radius:12px;padding:12px 18px;max-width:92vw;display:none;z-index:9;font-weight:600}
.toast.show{display:block} .toast.bad{border-color:var(--bad);border-style:dashed}
.foot{color:var(--dim);font-size:.85rem;margin-top:18px}
.also li{margin:4px 0;color:#c9d1d9}
.legend{color:var(--dim);font-size:.85rem;margin:-4px 0 10px}
</style></head>
<body>
<h1>Pi-hole switches <small id="updated">loading...</small></h1>
<div class="chips" id="chips"></div>
<div class="legend">Shapes: <b>&#9632; ON</b> = blocking &nbsp; <b>&#10074;&#10074; PAUSED</b> = override running &nbsp; <b>&#9675; OFF</b> = not blocking now. Porn + ads can never be paused here.</div>
<div class="err" id="err"></div>

<h2>Groups (the switches)</h2>
<div class="grid" id="groups"></div>

<h2>Also on</h2>
<ul class="also" id="also"></ul>

<h2>Devices and their groups</h2>
<div class="clients" id="clients"></div>

<h2>Admin password lock</h2>
<div class="card lock" id="lock"></div>

<h2>Blocked requests</h2>
<div class="filters">
 <div><label class="legend" for="fclient">Device</label><select id="fclient"><option value="">All devices</option></select></div>
 <div><label class="legend">Window</label><div class="seg" id="fwin">
   <button data-m="60">1 h</button><button data-m="360">6 h</button><button data-m="1440" class="sel">24 h</button></div></div>
 <div><label class="legend" for="fdom">Domain contains</label><input id="fdom" placeholder="e.g. amazon" autocomplete="off" autocapitalize="off" spellcheck="false"></div>
</div>
<div class="count" id="count"></div>
<table><thead><tr><th>When</th><th>Domain</th><th>Device</th><th>Why blocked</th></tr></thead><tbody id="rows"></tbody></table>
<div class="foot">Auto-refresh every 30 s (paused while you type). Changes go only through <code>pihole-pause</code> (max 240 min, reason required, Telegram-notified, auto re-enabled) and <code>pihole-unlock</code>.</div>
<div class="toast" id="toast"></div>

<script>
const $ = s => document.querySelector(s);
const esc = s => String(s ?? "").replace(/[&<>"']/g, c => ({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#39;"}[c]));
let S = null, win = 1440, typing = false, openForm = null, lastBlocked = null;
const hm = d => d.toLocaleTimeString([], {hour:"2-digit", minute:"2-digit"});
const dt = ts => { const d = new Date(ts*1000); const today = new Date().toDateString() === d.toDateString();
  return (today ? "" : d.toLocaleDateString([], {day:"numeric", month:"short"}) + " ") + d.toLocaleTimeString([], {hour:"2-digit", minute:"2-digit", second:"2-digit"}); };
function left(iso){ const ms = new Date(iso) - Date.now(); if (ms <= 0) return "ending now";
  const s = Math.floor(ms/1000), h = Math.floor(s/3600), m = Math.floor(s%3600/60), x = s%60;
  return (h ? h + " h " : "") + String(m).padStart(h?2:1,"0") + " min " + String(x).padStart(2,"0") + " s left"; }
function toast(msg, bad){ const t = $("#toast"); t.textContent = msg; t.className = "toast show" + (bad ? " bad" : "");
  clearTimeout(toast.t); toast.t = setTimeout(() => t.className = "toast", bad ? 7000 : 3500); }
async function j(url, opt){ const r = await fetch(url, opt); let b = {}; try { b = await r.json(); } catch(e) {}
  if (!r.ok || b.error) throw new Error(b.error || ("HTTP " + r.status)); return b; }
const post = (url, body) => j(url, {method:"POST", headers:{"Content-Type":"application/json"}, body:JSON.stringify(body)});

function renderChips(){
  const b = S.blocking === "enabled", L = S.lock;
  let h = `<div class="chip ${b ? "on" : "off"}"><span class="ico">${b ? "&#9632;" : "&#9675;"}</span><div>Blocking ${b ? "ON" : "OFF"}<small>global Pi-hole switch${S.blocking_timer ? " (timer " + S.blocking_timer + " s)" : ""}</small></div></div>`;
  if (!L) h += `<div class="chip off"><span class="ico">?</span><div>Lock: unknown</div></div>`;
  else if (L.pending) h += `<div class="chip pend"><span class="ico">&#9203;</span><div>Lock: release pending<small>password reaches Telegram at ${hm(new Date(L.releases))}</small></div></div>`;
  else h += `<div class="chip on"><span class="ico">&#128274;</span><div>Lock: locked<small>admin password sealed</small></div></div>`;
  const paused = S.groups.filter(g => g.pause).length;
  h += `<div class="chip ${paused ? "pend" : "on"}"><span class="ico">${paused ? "&#10074;&#10074;" : "&#10003;"}</span><div>${paused} override${paused === 1 ? "" : "s"} running<small>${S.groups.filter(g => g.enabled).length} of ${S.groups.length} groups ON</small></div></div>`;
  $("#chips").innerHTML = h;
}
function stateOf(g){ if (g.pause) return ["paused", "&#10074;&#10074;", "PAUSED"]; return g.enabled ? ["on", "&#9632;", "ON"] : ["off", "&#9675;", "OFF"]; }
function renderGroups(){
  const out = S.groups.map(g => {
    const [cls, ico, word] = stateOf(g);
    let extra = "";
    if (g.pause) extra = `<div class="pausebox">Paused until <b>${hm(new Date(g.pause.until))}</b> &middot; <b class="cd" data-until="${esc(g.pause.until)}">${left(g.pause.until)}</b><br>Reason: ${esc(g.pause.reason)}<br><span class="legend">by ${esc(g.pause.by)}${g.pause.since ? " at " + hm(new Date(g.pause.since)) : ""}</span></div>`;
    else if (!g.enabled && g.name.startsWith("bedtime")) extra = `<div class="legend">Off now: the bedtime engine turns it on at 00:00 and off at 06:00.</div>`;
    let btns = "";
    if (g.pausable) {
      btns = `<div class="btns">
        ${g.name === "always-shop"  /* his 10-23: Amazon gets a 15-20 min window for real shopping */
          ? `<button class="warn" onclick="showForm('${g.name}',20)">&#10074;&#10074; Shop 20 min</button>`
          : `<button class="warn" onclick="showForm('${g.name}',30)">&#10074;&#10074; Pause 30 min</button>
        <button class="warn" onclick="showForm('${g.name}',120)">&#10074;&#10074; Pause 2 h</button>`}
        <button onclick="showForm('${g.name}',60)">Pause&hellip;</button>
        ${g.pause ? `<button class="primary" onclick="resume('${g.name}')">&#9654; Resume now</button>` : ""}
      </div>
      <div class="form ${openForm === g.name ? "open" : ""}" id="form-${g.name}">
        <div class="row"><div><label for="min-${g.name}">Minutes (1-240)</label>
          <input id="min-${g.name}" type="number" inputmode="numeric" min="1" max="240" value="60"></div></div>
        <div><label for="why-${g.name}">Reason (required, goes to the log and Telegram)</label>
          <textarea id="why-${g.name}" placeholder="e.g. claude agent researching a part on Amazon"></textarea></div>
        <div class="row"><button class="warn" onclick="pause('${g.name}')">&#10074;&#10074; Pause now</button>
          <button onclick="hideForm()">Cancel</button></div>
      </div>`;
    } else btns = `<div class="legend">&#128274; Read-only: never pausable.</div>`;
    return `<div class="card st-${cls}"><div class="top"><div><div class="title">${esc(g.title)}</div><div class="gname">group: ${esc(g.name)} (#${g.id})</div></div>
      <div class="state"><span class="ico">${ico}</span>${word}</div></div><div class="desc">${esc(g.description)}</div>${extra}${btns}</div>`;
  });
  // keep any half-typed reason across a re-render
  const keep = openForm ? {m: $("#min-" + openForm)?.value, w: $("#why-" + openForm)?.value} : null;
  $("#groups").innerHTML = out.join("");
  if (keep && openForm) { if ($("#min-" + openForm)) $("#min-" + openForm).value = keep.m; if ($("#why-" + openForm)) $("#why-" + openForm).value = keep.w; }
  bindTyping();
}
function renderRest(){
  $("#also").innerHTML = S.also_on.map(x => `<li>${esc(x)}</li>`).join("");
  const enabled = Object.fromEntries(S.groups.map(g => [g.name, g.enabled]));
  $("#clients").innerHTML = S.clients.length ? S.clients.map(c => `<div class="client"><b>${esc((c.comment || "").split(" \u2014 ")[0] || c.name || c.client)}</b>
    <div class="legend">${esc(c.client)}${c.name ? " &middot; " + esc(c.name) : ""}${c.ips.length ? " &middot; " + c.ips.map(esc).join(", ") : ""}</div>
    ${c.groups.map(n => `<span class="tag ${enabled[n] ? "" : "dis"}">${enabled[n] ? "&#9632;" : "&#9675;"} ${esc(n)}</span>`).join("")}</div>`).join("")
    : `<div class="legend">No per-device entries: every device is only in Default (porn + ads).</div>`;
  $("#clients").insertAdjacentHTML("beforeend", `<div class="legend">Any device not listed above gets only Default (porn + ads lists).</div>`);
  const L = S.lock;
  $("#lock").innerHTML = !L ? `<div>Lock status unavailable.</div>` : (L.pending
    ? `<div><b>&#9203; Release pending.</b> Requested ${esc(new Date(L.requested).toLocaleString())}; the password goes to Telegram at <b>${esc(new Date(L.releases).toLocaleString())}</b>.</div>
       <div class="btns"><button class="danger" onclick="unlock('cancel')">&#10006; Cancel the request</button></div>`
    : `<div><b>&#128274; Locked.</b> The Pi-hole admin password is sealed. Asking for it starts a 6-hour wait, then it is sent to Telegram.</div>
       <div class="btns"><button class="warn" onclick="unlock('request')">&#9203; Request admin password (6 h)</button></div>`)
    + (L && L.log.length ? `<div class="log">${L.log.map(esc).join("\n")}</div>` : "");
  const sel = $("#fclient"), cur = sel.value;
  sel.innerHTML = `<option value="">All devices</option>` + S.known_ips.map(d => `<option value="${esc(d.ip)}">${esc(d.name)} (${esc(d.ip)})</option>`).join("");
  sel.value = cur;
  const errs = S.errors || [];
  $("#err").className = "err" + (errs.length ? " show" : ""); $("#err").textContent = errs.length ? "Some reads failed: " + errs.join(" | ") : "";
}
function showForm(name, m){ openForm = name; renderGroups(); const i = $("#min-" + name); i.value = m; const w = $("#why-" + name);
  if (name === "always-shop" && !w.value) w.value = "shopping"; w.focus(); }
function hideForm(){ openForm = null; renderGroups(); }
async function pause(name){
  const m = parseInt($("#min-" + name).value, 10), w = $("#why-" + name).value.trim();
  if (!(m >= 1 && m <= 240)) return toast("Minutes must be 1 to 240", true);
  if (w.length < 3) { $("#why-" + name).focus(); return toast("Write a reason first (it goes to Telegram)", true); }
  try { const r = await post("/api/pause", {group:name, minutes:m, reason:w});
        toast(`${name} paused until ${hm(new Date(r.result.until))}`); openForm = null; await load(); }
  catch(e) { toast("Pause failed: " + e.message, true); }
}
async function resume(name){
  if (!confirm(`Turn ${name} back ON now?`)) return;
  try { await post("/api/resume", {group:name}); toast(name + " is ON again"); await load(); }
  catch(e) { toast("Resume failed: " + e.message, true); }
}
async function unlock(action){
  if (!confirm(action === "request" ? "Start the 6-hour release of the Pi-hole admin password to Telegram?" : "Cancel the pending password release?")) return;
  try { const r = await post("/api/unlock", {action}); toast((r.message || []).join(" ") || "done"); await load(); }
  catch(e) { toast("Failed: " + e.message, true); }
}
function renderBlocked(){
  const B = lastBlocked; if (!B) return;
  const f = $("#fdom").value.trim().toLowerCase();
  const rows = f ? B.rows.filter(r => (r.domain || "").toLowerCase().includes(f)) : B.rows;
  const wl = {60:"last hour", 360:"last 6 h", 1440:"last 24 h"}[B.minutes] || B.minutes + " min";
  $("#count").innerHTML = `<b>${rows.length}</b> blocked request${rows.length === 1 ? "" : "s"} shown, ${wl}${B.count >= 200 ? " (newest 200)" : ""}${B.errors.length ? " &middot; some statuses failed to load" : ""}`;
  $("#rows").innerHTML = rows.map((r, i) => `<tr><td class="t">${dt(r.time)}</td><td class="d">${esc(r.domain)}<div class="legend">${esc(r.type)}${r.cname ? " via " + esc(r.cname) : ""}</div></td>
    <td>${esc(r.client_name || "")}<div class="legend">${esc(r.client_ip)}</div></td>
    <td class="why">&#10006; ${esc(r.why)}${r.rule ? `<br><code>${esc(r.rule)}</code>` : ""}${r.groups.length ? `<br>${r.groups.map(n => `<span class="tag">${esc(n)}</span>`).join("")}` : ""}
     ${r.status.startsWith("GRAVITY") && !r.rule ? `<br><button class="linkbtn" data-domain="${esc(r.domain)}" onclick="whyList(this)">Which list?</button><div class="whyout"></div>` : ""}</td></tr>`).join("")
    || `<tr><td colspan="4" class="legend">Nothing blocked in this window.</td></tr>`;
}
async function whyList(btn){
  const dom = btn.dataset.domain; btn.disabled = true; const out = btn.nextElementSibling;
  try { const r = await j("/api/why?domain=" + encodeURIComponent(dom));
        out.innerHTML = r.hits.length ? r.hits.map(h => `&bull; ${esc(h.name)}`).join("<br>") : "not in any list now (list updated since?)"; }
  catch(e) { out.textContent = "lookup failed: " + e.message; }
}
async function loadBlocked(){
  const c = $("#fclient").value, f = $("#fdom").value.trim().replace(/[^A-Za-z0-9._-]/g, "");
  try { lastBlocked = await j(`/api/blocked?minutes=${win}&limit=200${c ? "&client=" + encodeURIComponent(c) : ""}${f ? "&domain=" + encodeURIComponent(f) : ""}`); renderBlocked(); }
  catch(e) { $("#count").textContent = "Blocked list failed: " + e.message; }
}
async function load(){
  try { S = await j("/api/state"); renderChips(); renderGroups(); renderRest();
        $("#updated").textContent = "updated " + new Date().toLocaleTimeString(); }
  catch(e) { const el = $("#err"); el.className = "err show"; el.textContent = "Cannot read state: " + e.message; }
  await loadBlocked();
}
function bindTyping(){
  document.querySelectorAll("input,textarea,select").forEach(el => { if (el.dataset.b) return; el.dataset.b = 1;
    el.addEventListener("focus", () => typing = true); el.addEventListener("blur", () => typing = false); });
}
$("#fwin").addEventListener("click", e => { const b = e.target.closest("button"); if (!b) return;
  win = +b.dataset.m; document.querySelectorAll("#fwin button").forEach(x => x.classList.toggle("sel", x === b)); loadBlocked(); });
$("#fclient").addEventListener("change", loadBlocked);
$("#fdom").addEventListener("input", () => { renderBlocked(); clearTimeout(loadBlocked.t); loadBlocked.t = setTimeout(loadBlocked, 500); });
bindTyping();
setInterval(() => document.querySelectorAll(".cd").forEach(el => el.textContent = left(el.dataset.until)), 1000);
setInterval(() => { if (!typing && !openForm && !document.hidden) load(); }, 30000);
load();
</script>
</body></html>
"""


def main():
    srv = ThreadingHTTPServer(("0.0.0.0", PORT), Handler)
    srv.daemon_threads = True
    sys.stderr.write(f"pihole-ui listening on 0.0.0.0:{PORT} (tools in {BIN}, sudo={' '.join(SUDO) or 'none'})\n")
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
