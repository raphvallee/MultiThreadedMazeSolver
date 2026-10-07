"""Cross-check our clone against the live site on sampled probe paths."""

import base64, json, random, urllib.request

REMOTE = "https://daedalus.defi.info.cegepmontpetit.ca/move"
LOCAL = "http://127.0.0.1:8173/move"

state = json.load(open(r"tools\remote_map\state.json"))
paths = {tuple(map(int, k.split(","))): v for k, v in state["paths"].items()}
probed = {tuple(map(int, k.split(","))): p for k, p in state["probed"].items()}
solution = paths[(119, 118)]
minotaur = paths[(68, 68)]

facts = []
for pos, letters in probed.items():
    for letter, outcome in letters.items():
        facts.append((paths[pos] + letter, outcome))

random.seed(42)
sample = random.sample(facts, 30)
cases = (
    [("", "well")]
    + sample
    + [
        (solution, "flag"),
        (minotaur, "minotaur"),
        (minotaur + "d", "well"),  # stepping off the minotaur is a normal move
    ]
)

EXPECT_LEN = {"bonk": 1730, "well": 1744, "minotaur": 1774}


def length(url, path):
    cookie = base64.b64encode(path.encode()).decode()
    req = urllib.request.Request(url, headers={"Cookie": f"path={cookie}"})
    for attempt in range(5):
        try:
            with urllib.request.urlopen(req, timeout=30) as r:
                return len(r.read())
        except Exception:
            if attempt == 4:
                raise
            import time

            time.sleep(0.5 * (attempt + 1))


mismatch = 0
for path, expect in cases:
    rn = length(REMOTE, path)
    ln = length(LOCAL, path)
    ok = rn == ln
    if expect in EXPECT_LEN:
        ok = ok and rn == EXPECT_LEN[expect]
    if not ok:
        mismatch += 1
    tag = "OK" if ok else "MISMATCH"
    if not ok or len(path) < 3:
        print(f"len={len(path):5d} remote={rn} local={ln} expect={expect} {tag}")
print(f"checked {len(cases)} paths, mismatches: {mismatch}")
