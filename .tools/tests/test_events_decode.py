#!/usr/bin/env python3
"""Consistency checks for the world-event sheets written by .tools/events_decode.py.

Reads only the committed data/*.tsv files (no game data needed). Run from the repository root:
    python3 .tools/tests/test_events_decode.py
"""
import collections
from pathlib import Path

DATA = Path(__file__).resolve().parents[2] / "data"


def rows(name):
    lines = [l.rstrip("\n").split("\t") for l in open(DATA / f"{name}.tsv", encoding="utf-8") if not l.startswith("#")]
    head = [c.split(":")[0] for c in lines[0]]
    return [dict(zip(head, r)) for r in lines[1:]]


def test_distortion_links():
    sites, sp = rows("world_events_distortion_sites"), rows("world_events_distortion_spawners")
    enc, items = rows("world_events_distortion_encounters"), rows("world_events_distortion_items")
    assert len(sites) == 116 and len(sp) == 464
    names = {(r["area"], r["spawner_name"]) for r in sp}
    for s in sites:
        for n in s["spawners"].split(";"):
            assert (s["area"], n) in names, (s["name"], n)
    keys = {(r["area"], r["encount_key"]) for r in enc}
    assert all((r["area"], r["encount_key"]) in keys for r in sp)
    item_keys = {(r["area"], r["item_table_key"]) for r in items}
    assert all((s["area"], s["item_table_key"]) in item_keys for s in sites)
    for (_a, _k), tot in {(r["area"], r["item_table_key"]): int(r["total_weight"]) for r in items}.items():
        assert tot > 0
    print("[PASS] distortion sites -> spawners -> encounter tables / item tables")


def test_weather_and_time_limits():
    for r in rows("world_events_area_weather"):
        if r["area"].startswith("ha_area"):  # developer test maps (ha_test_*) do not sum to 100
            assert int(r["total"]) == 100, r["area"]
    assert sum(int(r["weight"]) for r in rows("world_events_mmo_time_limits")) == 100
    print("[PASS] game-area weather weights and massive-outbreak time-limit weights sum to 100")


def test_outbreak_ids():
    areas = {r["area"]: int(r["outbreak_count"]) for r in rows("world_events_outbreak_areas")}
    ids = collections.defaultdict(set)
    for r in rows("world_events_outbreak_spawners"):
        ids[r["area"]].add(int(r["outbreak_id"]))
    for a, n in areas.items():
        assert ids[a] == set(range(1, n + 1)), a
    print("[PASS] one outbreak spawner per outbreak id, ids 1..outbreak_count")


def test_spawn_conditions():
    sc = rows("field_spawn_conditions")
    table = {(r["area"], r["spawner_id"]) for r in rows("field_spawn_table")}
    assert {(r["area"], r["spawner_id"]) for r in sc} == table
    # Known day/night facts: ghosts at night, Silcoon by day / Cascoon by night.
    gastly = collections.Counter((r["time_day"], r["time_night"]) for r in sc if r["species"] == "gastly")
    assert gastly[("0", "1")] > 10 * gastly[("1", "1")], gastly
    sd = {r["species"]: r for r in rows("world_events_species_time_weather")}
    assert sd["silcoon"]["time_night"] == "0" and sd["cascoon"]["time_day"] == "0"
    assert float(sd["ponyta"]["weather_drought"]) > 1 and sd["ponyta"]["weather_rain"] == "0"
    print("[PASS] spawn conditions cover every spawner; day/night and weather spot checks")


if __name__ == "__main__":
    test_distortion_links()
    test_weather_and_time_limits()
    test_outbreak_ids()
    test_spawn_conditions()
