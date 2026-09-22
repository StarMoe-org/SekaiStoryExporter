#!/usr/bin/env python3
"""Corpus statistics over unpacked PJSK scenario JSON files.

Answers the questions that decide project scope:
  - which SnippetAction / SpecialEffectType values actually occur, and how often
  - how long the tail is (does covering N effect types cover 95% of occurrences?)
  - which scenarios use branches, movies, or other unsupported features
  - the full field set of each data array, so the IR can be designed against reality

Usage:
    python3 tools/stats/scenario_stats.py <dir-with-scenario-json> [...]
"""

import collections
import json
import pathlib
import sys


def load(paths):
    for root in paths:
        root = pathlib.Path(root)
        files = sorted(root.glob("*.json")) if root.is_dir() else [root]
        for f in files:
            try:
                d = json.loads(f.read_text(encoding="utf-8"))
            except Exception as exc:  # noqa: BLE001 - report and continue
                print(f"skip {f}: {exc}", file=sys.stderr)
                continue
            if "Snippets" in d:
                yield f, d


ARRAY_OF_ACTION = {1: "TalkData", 2: "LayoutData", 4: "LayoutData",
                   6: "SpecialEffectData", 7: "SoundData"}


def main(argv):
    if len(argv) < 2:
        print(__doc__)
        return 1

    action = collections.Counter()
    progress = collections.Counter()
    effect = collections.Counter()
    effect_eps = collections.defaultdict(set)
    layout_type = collections.Counter()
    play_mode = collections.Counter()
    fields = collections.defaultdict(set)
    flags = collections.Counter()
    bundles = collections.Counter()
    costumes = set()
    episodes = 0
    snippets = 0

    for f, d in load(argv[1:]):
        episodes += 1
        snippets += len(d["Snippets"])
        for s in d["Snippets"]:
            action[s["Action"]] += 1
            progress[s["ProgressBehavior"]] += 1
            if s["Delay"]:
                flags["snippet_with_delay"] += 1
        for e in d.get("SpecialEffectData", []):
            effect[e["EffectType"]] += 1
            effect_eps[e["EffectType"]].add(f.name)
            fields["SpecialEffectData"].update(e)
        for lay in d.get("LayoutData", []):
            layout_type[lay.get("Type")] += 1
            fields["LayoutData"].update(lay)
        for snd in d.get("SoundData", []):
            play_mode[snd["PlayMode"]] += 1
            fields["SoundData"].update(snd)
        for t in d.get("TalkData", []):
            fields["TalkData"].update(t)
            flags["talk_total"] += 1
            if t.get("LipSync"):
                flags["talk_lipsync_on"] += 1
            if not t.get("Voices"):
                flags["talk_without_voice"] += 1
            if t.get("Motions"):
                flags["talk_with_inline_motions"] += 1
        for b in d.get("NeedBundleNames", []):
            bundles[b.split("/")[1] if "/" in b else b] += 1
        for c in d.get("AppearCharacters", []):
            costumes.add((c["Character2dId"], c["CostumeType"]))

    print(f"episodes {episodes}   snippets {snippets}\n")

    def dist(title, counter, note=None):
        print(f"{title}  (distinct {len(counter)})")
        total = sum(counter.values())
        acc = 0
        for k, v in counter.most_common():
            acc += v
            extra = f"  in {len(effect_eps[k]):>3}/{episodes} eps" if note == "eps" else ""
            print(f"  {k!s:>4} : {v:6}  {v / total:6.1%}  cum {acc / total:6.1%}{extra}")
        print()

    dist("SnippetAction", action)
    dist("ProgressBehavior", progress)
    dist("SpecialEffectType", effect, note="eps")
    dist("LayoutData.Type", layout_type)
    dist("SoundData.PlayMode", play_mode)

    print("TalkData flags")
    for k, v in sorted(flags.items()):
        print(f"  {k:28} {v}")
    print("\nField sets")
    for arr, keys in sorted(fields.items()):
        print(f"  {arr}: {sorted(keys)}")
    print(f"\nCostumes ({len(costumes)})")
    for c in sorted(costumes):
        print(f"  {c[0]:>4}  {c[1]}")
    print("\nReferenced bundle categories")
    for k, v in bundles.most_common():
        print(f"  {k:14} {v}")

    # Consistency check: each Action must index exactly one data array.
    print("\nAction -> array length consistency")
    for act, arr in sorted(ARRAY_OF_ACTION.items()):
        print(f"  Action {act} -> {arr}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
