#!/usr/bin/env python3
"""Analyze STG event format by testing different parsing theories."""

import struct
import sys
from pathlib import Path

def read_u32(data, offset):
    return struct.unpack_from('<I', data, offset)[0]

def read_u16(data, offset):
    return struct.unpack_from('<H', data, offset)[0]

def read_cstr(data, offset, maxlen):
    end = data.index(0, offset) if 0 in data[offset:offset+maxlen] else offset+maxlen
    return data[offset:end].decode('ascii', errors='replace')

def parse_stg_tail_start(data):
    """Parse the tail section to find where events start."""
    header_size = 628
    unit_size = 544
    unit_count = read_u32(data, 0x270)  # unitCount in header at offset 0x270

    tail_offset = header_size + unit_count * unit_size
    print(f"  Units: {unit_count}, tail starts at 0x{tail_offset:X}")

    # Areas
    area_count = read_u32(data, tail_offset)
    area_section_size = 4 + area_count * 84
    print(f"  Areas: {area_count} ({area_section_size} bytes)")

    # Variables
    var_offset = tail_offset + area_section_size
    var_count = read_u32(data, var_offset)
    var_section_size = 4 + var_count * 76
    print(f"  Variables: {var_count} ({var_section_size} bytes)")
    for i in range(var_count):
        vname = read_cstr(data, var_offset + 4 + i * 76, 64)
        print(f"    var[{i}]: '{vname}'")

    # Mystery fields
    mystery_offset = var_offset + var_section_size
    m1 = read_u32(data, mystery_offset)
    m2 = read_u32(data, mystery_offset + 4)
    print(f"  Mystery fields: {m1}, {m2}")

    # Event count
    event_count_offset = mystery_offset + 8
    event_count = read_u32(data, event_count_offset)
    events_offset = event_count_offset + 4
    print(f"  Event count: {event_count}")
    print(f"  Events start at: 0x{events_offset:X}")

    return events_offset, event_count

def find_event_descriptions(data, start, end):
    """Find offsets of strings that look like event descriptions."""
    # Search for readable ASCII sequences that could be descriptions
    offsets = []
    i = start
    while i < end:
        # Look for "stage" keyword
        if data[i:i+5] == b'stage':
            offsets.append(i)
        i += 1
    return offsets

def try_parse_fixed16(data, events_offset, event_count, file_size):
    """Theory 1: Fixed 16-byte entries, format:
    desc(64) + blockId(4) + numCond(4) + conds[N*16] + numAct(4) + acts[M*16]
    """
    print("\n=== Theory 1: Fixed 16-byte entries ===")
    print("  desc(64) + blockId(4) + numCond(4) + conds[N*16] + numAct(4) + acts[M*16]")

    offset = events_offset
    for i in range(min(event_count, 10)):
        if offset + 76 > file_size:
            print(f"  Event {i}: OUT OF BOUNDS at 0x{offset:X}")
            break

        desc = read_cstr(data, offset, 64)
        blockId = read_u32(data, offset + 64)
        numCond = read_u32(data, offset + 68)

        if numCond > 100:
            print(f"  Event {i} at 0x{offset:X}: desc='{desc[:30]}' blockId={blockId} numCond={numCond} INVALID")
            break

        cond_end = offset + 72 + numCond * 16
        if cond_end + 4 > file_size:
            print(f"  Event {i}: conditions extend past EOF")
            break

        numAct = read_u32(data, cond_end)

        if numAct > 100:
            print(f"  Event {i} at 0x{offset:X}: desc='{desc[:30]}' blockId={blockId} numCond={numCond} numAct={numAct} INVALID")
            break

        event_size = 72 + numCond * 16 + 4 + numAct * 16

        # Show condition/action details
        conds = []
        for c in range(numCond):
            cid = read_u32(data, offset + 72 + c * 16)
            p1 = read_u32(data, offset + 72 + c * 16 + 4)
            p2 = read_u32(data, offset + 72 + c * 16 + 8)
            p3 = read_u32(data, offset + 72 + c * 16 + 12)
            conds.append(f"({cid},{p1},{p2},{p3})")

        acts = []
        act_base = cond_end + 4
        for a in range(numAct):
            aid = read_u32(data, act_base + a * 16)
            p1 = read_u32(data, act_base + a * 16 + 4)
            p2 = read_u32(data, act_base + a * 16 + 8)
            p3 = read_u32(data, act_base + a * 16 + 12)
            acts.append(f"({aid},{p1},{p2},{p3})")

        next_offset = offset + event_size
        print(f"  Event {i} at 0x{offset:X}: size={event_size} desc='{desc[:30]}' blockId={blockId}")
        print(f"    numCond={numCond} conds={' '.join(conds)}")
        print(f"    numAct={numAct} acts={' '.join(acts)}")
        print(f"    next at 0x{next_offset:X}")

        offset = next_offset

    return offset

def try_parse_adjacent_counts(data, events_offset, event_count, file_size):
    """Theory 2: Adjacent counts, format:
    desc(64) + blockId(4) + numCond(4) + numAct(4) + conds[N*16] + acts[M*16]
    """
    print("\n=== Theory 2: Adjacent numCond/numAct + fixed 16-byte entries ===")

    offset = events_offset
    for i in range(min(event_count, 10)):
        if offset + 76 > file_size:
            print(f"  Event {i}: OUT OF BOUNDS at 0x{offset:X}")
            break

        desc = read_cstr(data, offset, 64)
        blockId = read_u32(data, offset + 64)
        numCond = read_u32(data, offset + 68)
        numAct = read_u32(data, offset + 72)

        if numCond > 50 or numAct > 50:
            print(f"  Event {i} at 0x{offset:X}: desc='{desc[:30]}' blockId={blockId} numCond={numCond} numAct={numAct} INVALID")
            break

        event_size = 76 + (numCond + numAct) * 16
        next_offset = offset + event_size

        # Show first condition if any
        entry_base = offset + 76
        conds_str = ""
        for c in range(min(numCond, 3)):
            cid = read_u32(data, entry_base + c * 16)
            conds_str += f" ({cid},...)"
        acts_str = ""
        act_base = entry_base + numCond * 16
        for a in range(min(numAct, 3)):
            aid = read_u32(data, act_base + a * 16)
            acts_str += f" ({aid},...)"

        print(f"  Event {i} at 0x{offset:X}: size={event_size} desc='{desc[:30]}' blockId={blockId}")
        print(f"    numCond={numCond}{conds_str} numAct={numAct}{acts_str}")
        print(f"    next at 0x{next_offset:X}")

        offset = next_offset

    return offset

def try_parse_self_describing(data, events_offset, event_count, file_size):
    """Theory 3: Self-describing entries with paramCount:
    desc(64) + blockId(4) + numCond(4) + conds + numAct(4) + acts
    where each entry = id(4) + paramCount(4) + params[paramCount*4]
    """
    print("\n=== Theory 3: Self-describing entries (id + paramCount + params) ===")

    offset = events_offset
    for i in range(min(event_count, 10)):
        if offset + 76 > file_size:
            print(f"  Event {i}: OUT OF BOUNDS at 0x{offset:X}")
            break

        desc = read_cstr(data, offset, 64)
        blockId = read_u32(data, offset + 64)
        numCond = read_u32(data, offset + 68)

        if numCond > 50:
            print(f"  Event {i} at 0x{offset:X}: numCond={numCond} INVALID")
            break

        pos = offset + 72
        conds = []
        valid = True
        for c in range(numCond):
            if pos + 8 > file_size:
                valid = False
                break
            cid = read_u32(data, pos)
            pc = read_u32(data, pos + 4)
            if pc > 20:
                print(f"  Event {i}: cond[{c}] id={cid} paramCount={pc} INVALID")
                valid = False
                break
            params = [read_u32(data, pos + 8 + p*4) for p in range(pc)]
            conds.append(f"(id={cid},pc={pc},params={params})")
            pos += 8 + pc * 4

        if not valid:
            break

        if pos + 4 > file_size:
            print(f"  Event {i}: numAct field past EOF")
            break

        numAct = read_u32(data, pos)
        pos += 4

        if numAct > 50:
            print(f"  Event {i} at 0x{offset:X}: numAct={numAct} INVALID")
            break

        acts = []
        for a in range(numAct):
            if pos + 8 > file_size:
                valid = False
                break
            aid = read_u32(data, pos)
            pc = read_u32(data, pos + 4)
            if pc > 20:
                print(f"  Event {i}: act[{a}] id={aid} paramCount={pc} INVALID")
                valid = False
                break
            params = [read_u32(data, pos + 8 + p*4) for p in range(pc)]
            acts.append(f"(id={aid},pc={pc},params={params})")
            pos += 8 + pc * 4

        if not valid:
            break

        event_size = pos - offset
        print(f"  Event {i} at 0x{offset:X}: size={event_size} desc='{desc[:30]}' blockId={blockId}")
        print(f"    numCond={numCond} {' '.join(conds)}")
        print(f"    numAct={numAct} {' '.join(acts)}")
        print(f"    next at 0x{pos:X}")

        offset = pos


def try_parse_lookup_paramcount(data, events_offset, event_count, file_size):
    """Theory 4: Lookup-based param count (no paramCount field):
    desc(64) + blockId(4) + numCond(4) + conds + numAct(4) + acts
    where each entry = id(4) + params[lookup(id)*4]
    Param count is determined by the condition/action ID.
    """
    print("\n=== Theory 4: Lookup-based param count (no paramCount in entry) ===")

    # Condition param counts from stg_script_catalog.h
    cond_params = {
        0: 0,   # CON_STAGE_START
        1: 1,   # CON_ALL_DEAD
        2: 1,   # CON_LEADER_DEAD
        3: 2,   # CON_HP_UNDER
        4: 2,   # CON_TROOP_NUM_UNDER
        5: 2,   # CON_AREA
        6: 1,   # CON_TIME_ELAPSED
        7: 1,   # CON_ITEM_GET
        8: 1,   # CON_ALL_AREA
        9: 3,   # CON_TROOP_NEAR
        10: 2,  # CON_TROOP_ATTACK
        11: 1,  # CON_OBJECT_DEAD
        12: 2,  # CON_TROOP_NOT_AREA
        13: 1,  # CON_ANY_DEAD
        14: 2,  # CON_LEADER_HP_UNDER
        15: 2,  # CON_ALL_TROOP_NEAR
        16: 1,  # CON_EVENT_TRIGGERED
        17: 1,  # CON_TROOP_HERO
        18: 2,  # CON_OBJECT_HP_UNDER
        19: 3,  # CON_VAR_INT_COMPARE
        20: 1,  # CON_TROOP_EXIST
    }

    # Action param counts (from catalog - count non-empty paramNames)
    act_params = {}
    # Build from stg_script_catalog.h data
    act_data = [
        (0, 0), (1, 1), (2, 1), (3, 0), (4, 0), (5, 0), (6, 1), (7, 2),
        (8, 2), (9, 2), (10, 2), (11, 2), (12, 1), (13, 1), (14, 1),
        (15, 1), (16, 1), (17, 1), (18, 1), (19, 1), (20, 1), (21, 3),
        (22, 1), (23, 1), (24, 3), (25, 2), (26, 1), (27, 1), (28, 2),
        (29, 1), (30, 0), (31, 1), (32, 1), (33, 1), (34, 2), (35, 2),
        (36, 2), (37, 2), (38, 2), (39, 3), (40, 2), (41, 2), (42, 2),
        (43, 2), (44, 2), (45, 0), (46, 0), (47, 1), (48, 0), (49, 0),
        (50, 1), (51, 1), (52, 2), (53, 2), (54, 1), (55, 2), (56, 2),
        (57, 1), (58, 1), (59, 1), (60, 1), (61, 2), (62, 1), (63, 1),
        (64, 1), (65, 2), (66, 2), (67, 2), (68, 2), (69, 2), (70, 1),
        (71, 2), (72, 1), (73, 0), (74, 2), (75, 2), (76, 2), (77, 2),
        (78, 2), (79, 1), (80, 3), (81, 2), (82, 2), (83, 2), (84, 1),
        (85, 2), (86, 2), (87, 1), (88, 2), (89, 1), (90, 1), (91, 1),
        (92, 1), (93, 2), (94, 1), (95, 2), (96, 1), (97, 1), (98, 1),
        (99, 2), (100, 2), (101, 2), (102, 2), (103, 2), (104, 3),
        (105, 2), (106, 2), (145, 1), (146, 1),
    ]
    for aid, pc in act_data:
        act_params[aid] = pc

    offset = events_offset
    for i in range(min(event_count, 10)):
        if offset + 76 > file_size:
            print(f"  Event {i}: OUT OF BOUNDS at 0x{offset:X}")
            break

        desc = read_cstr(data, offset, 64)
        blockId = read_u32(data, offset + 64)
        numCond = read_u32(data, offset + 68)

        if numCond > 50:
            print(f"  Event {i} at 0x{offset:X}: numCond={numCond} INVALID")
            break

        pos = offset + 72
        conds = []
        valid = True
        for c in range(numCond):
            if pos + 4 > file_size:
                valid = False
                break
            cid = read_u32(data, pos)
            pc = cond_params.get(cid)
            if pc is None:
                print(f"  Event {i}: cond[{c}] UNKNOWN id={cid}")
                valid = False
                break
            params = [read_u32(data, pos + 4 + p*4) for p in range(pc)]
            conds.append(f"(id={cid},params={params})")
            pos += 4 + pc * 4

        if not valid:
            break

        if pos + 4 > file_size:
            print(f"  Event {i}: numAct field past EOF")
            break

        numAct = read_u32(data, pos)
        pos += 4

        if numAct > 100:
            print(f"  Event {i} at 0x{offset:X}: numAct={numAct} INVALID")
            break

        acts = []
        for a in range(numAct):
            if pos + 4 > file_size:
                valid = False
                break
            aid = read_u32(data, pos)
            pc = act_params.get(aid)
            if pc is None:
                print(f"  Event {i}: act[{a}] UNKNOWN id={aid} at 0x{pos:X}")
                valid = False
                break
            params = [read_u32(data, pos + 4 + p*4) for p in range(pc)]
            acts.append(f"(id={aid},params={params})")
            pos += 4 + pc * 4

        if not valid:
            break

        event_size = pos - offset
        print(f"  Event {i} at 0x{offset:X}: size={event_size} desc='{desc[:30]}' blockId={blockId}")
        print(f"    numCond={numCond} {' '.join(conds)}")
        print(f"    numAct={numAct} {' '.join(acts)}")
        print(f"    next at 0x{pos:X}")

        offset = pos


def analyze_file(filepath):
    data = Path(filepath).read_bytes()
    print(f"\nFile: {filepath} ({len(data)} bytes)")

    events_offset, event_count = parse_stg_tail_start(data)

    # Find "stage" descriptions for reference
    stage_offsets = []
    for i in range(events_offset, len(data) - 5):
        if data[i:i+5] == b'stage':
            stage_offsets.append(i)
    if stage_offsets:
        print(f"  'stage' found at: {['0x%X' % o for o in stage_offsets[:10]]}")

    try_parse_fixed16(data, events_offset, event_count, len(data))
    try_parse_adjacent_counts(data, events_offset, event_count, len(data))
    try_parse_self_describing(data, events_offset, event_count, len(data))
    try_parse_lookup_paramcount(data, events_offset, event_count, len(data))


if __name__ == '__main__':
    mission_dir = Path.home() / "Downloads" / "KUF Crusaders" / "Mission"

    # Analyze E1140 (simplest: 1 unit, 2 events)
    e1140 = mission_dir / "E1140.stg"
    if e1140.exists():
        analyze_file(e1140)
    else:
        print(f"NOT FOUND: {e1140}")

    # Analyze X3001 (complex: 46 units, 46 events)
    x3001 = mission_dir / "X3001.stg"
    if x3001.exists():
        analyze_file(x3001)
    else:
        print(f"NOT FOUND: {x3001}")
