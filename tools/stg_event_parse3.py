#!/usr/bin/env python3
"""Definitive STG event format analysis using E1140.stg (simplest file)."""

import struct
from pathlib import Path

def u32(data, off):
    return struct.unpack_from('<I', data, off)[0]

def f32(data, off):
    return struct.unpack_from('<f', data, off)[0]

def hexdump(data, offset, length, prefix=""):
    for i in range(0, length, 16):
        off = offset + i
        n = min(16, length - i, len(data) - off)
        if n <= 0: break
        hx = ' '.join(f'{data[off+j]:02X}' for j in range(n))
        asc = ''.join(chr(data[off+j]) if 32 <= data[off+j] < 127 else '.' for j in range(n))
        print(f'{prefix}0x{off:04X}: {hx:<48} {asc}')

# Correct param counts from MISSION_SCRIPTING.md / CruMission.h.
# Each entry is (param_count, has_string_param).
# String params are stored as null-terminated strings in the binary.
COND_PARAMS = {
    0: 2, 1: 3, 2: 2, 3: 2, 4: 2, 5: 3, 6: 4, 7: 2, 8: 1, 9: 1,
    10: 2, 11: 2, 12: 1, 13: 3, 14: 3, 15: 2, 17: 3, 18: 1, 19: 3,
    20: 2, 22: 2, 23: 2, 24: 2, 25: 2, 26: 2, 27: 0, 28: 1, 29: 1,
    30: 1, 31: 2, 32: 3, 33: 1, 34: 3, 35: 1, 36: 1, 37: 0, 38: 2,
    39: 2, 40: 1, 41: 1, 42: 0, 43: 1, 44: 2, 45: 4, 46: 4, 47: 1,
    48: 1, 49: 2, 50: 1, 51: 1, 52: 2, 53: 1, 54: 3, 55: 2, 56: 2,
    57: 1, 58: 1, 59: 2, 60: 1,
    # Worldmap conditions
    300: 1, 303: 0,
    402: 1, 403: 2, 404: 1, 405: 0, 406: 2, 407: 0, 408: 1, 409: 3,
    410: 1, 411: 1, 412: 0, 413: 0, 414: 0, 415: 2, 416: 2, 417: 1,
    418: 0, 419: 0, 420: 0, 421: 1,
}

# Actions with string params: {id: (total_params_including_string, string_position)}
# String params are variable length, so they break the fixed-param model.
STRING_ACTIONS = {110, 165, 172, 174, 180, 709, 729}

ACT_PARAMS = {
    0: 1, 1: 1, 2: 1, 3: 1, 4: 1, 5: 1, 6: 2, 7: 3, 8: 1, 9: 1,
    10: 2, 11: 2, 12: 3, 13: 3, 14: 1, 15: 2, 16: 2, 17: 2, 18: 2,
    19: 2, 20: 1, 21: 1, 22: 0, 23: 2, 24: 0, 26: 3, 27: 3, 28: 2,
    29: 0, 32: 2, 33: 4,  # ACT_VAR_DISPLAY: VarID, int, int, string - special!
    34: 1, 35: 1, 38: 1, 39: 1, 47: 0, 49: 0, 50: 0,
    51: 1, 52: 0, 53: 0, 54: 1, 55: 2, 56: 2, 57: 0, 58: 0, 59: 0,
    60: 2, 61: 1, 62: 2, 63: 1, 64: 1, 65: 1, 66: 2, 67: 1, 68: 1,
    70: 4, 71: 1, 72: 0, 73: 1, 74: 0, 75: 1, 76: 3, 77: 4, 78: 1,
    79: 0, 80: 1, 81: 0, 82: 2, 83: 1, 84: 1, 85: 0, 86: 2, 87: 3,
    88: 0, 89: 3, 90: 1, 91: 1, 92: 1, 93: 2, 94: 1, 95: 3, 96: 1,
    97: 1, 98: 1, 99: 3, 100: 2, 101: 1, 102: 1, 103: 0, 104: 0,
    105: 2, 106: 2, 107: 0, 109: 1, 110: -1,  # string action
    111: 1, 112: 1, 113: 1, 114: 1,
    115: 2, 116: 2, 117: 2, 118: 1, 119: 0, 120: 0, 121: 1, 122: 1,
    123: 1, 124: 1, 125: 1, 126: 1, 127: 1, 129: 2, 130: 1, 131: 3,
    132: 0, 133: 2, 135: 2, 136: 2, 137: -1,  # variable params
    138: 1, 139: 1, 140: 1, 141: 2,
    142: 3, 143: 3, 144: 2, 145: 1, 146: 1, 147: 1, 148: 0, 149: 0,
    150: -1,  # variable params
    151: -1,  # variable params
    152: 4, 153: 3, 154: 3, 155: 0, 157: 0, 158: 0, 159: 0, 160: -1,
    161: 2, 162: 1, 163: 0, 164: 0, 165: -1, 166: 1, 167: -1,
    168: 2, 169: 1, 170: 1, 171: 2,
    172: -1, 173: 1, 174: -1, 175: 2, 176: 2, 177: 2, 178: 2, 179: 0,
    180: -1, 181: 0, 182: 0,
    # Briefing
    500: 2, 501: 3, 502: 3, 503: 3, 504: 3, 505: 0, 506: 0, 507: 1,
    508: 1, 509: 1, 510: 0, 511: 4, 512: 1, 513: 3, 514: 4,
    # Live
    300: 1,
    # Worldmap
    700: 2, 701: 1, 702: 1, 703: 2, 704: 2, 706: 0, 707: 0, 708: 0,
    709: -1, 710: 0, 711: 2, 712: 2, 713: 0, 714: 0, 715: 1, 716: 1,
    717: 1, 718: 0, 719: 0, 720: 1, 721: 2, 723: 1, 724: 0, 725: 2,
    726: 3, 727: 1, 728: 3, 729: -1, 730: 1, 731: 2, 732: 3,
    733: 2, 734: 2, 735: 2, 736: 1, 737: 3, 738: 3, 739: 2,
}

COND_NAMES = {
    0: "CON_TIME_ELAPSED", 1: "CON_TIME_ELAPSED_FROM_MARKED", 2: "CON_TROOP_IN_AREA",
    3: "CON_TROOP_SCOUTER_STOPPED_IN_AREA", 4: "CON_TROOP_SCOUTER_IN_AREA",
    5: "CON_TROOP_SCOUTER_CLOSE_TO_TROOP", 6: "CON_TROOP_CLOSE_TO_TROOP",
    7: "CON_TROOP_TARGETED", 8: "CON_TROOP_ATTACKED", 9: "CON_TROOP_MELEE_ATTACKED",
    10: "CON_TROOP_ABILITY", 11: "CON_TROOP_WITH_THE_SUN_IN_BACK",
    12: "CON_LEADER_HAS_BEEN_KILLED", 13: "CON_STATE_HP_PERCENT",
    14: "CON_LEADER_HP", 15: "CON_OBJECT_LOCATED", 17: "CON_OBJECT_HP_PERCENT",
    18: "CON_OBJECT_DETECTED", 19: "CON_VAR", 20: "CON_PLAYER_TROOP_ALL_DISABLED",
    22: "CON_PLAYER_ALL_IN_AREA", 23: "CON_TROOP_NOT_IN_AREA",
    24: "CON_TROOP_ATTACKED_TROOP", 25: "CON_PLAYER_TROOP_NOT_IN_AREA",
    26: "CON_PLAYER_TROOP_IN_AREA", 27: "CON_ALWAYS_TRUE", 28: "CON_PLAYER_KO",
    29: "CON_SCOUTER_BACK", 30: "CON_TROOP_IN_SIGHT", 31: "CON_TROOP_TYPE",
    32: "CON_TROOP_TYPE_IN_AREA", 33: "CON_TROOP_NOT_ENGAGED",
    34: "CON_PLAYER_HP_SUM", 35: "CON_GOT_FIRE", 36: "CON_CAM_DIR",
    37: "CON_IS_DEMO_SKIPPED", 38: "CON_IS_MINE_AT", 39: "CON_IS_TRAP_AT",
    40: "CON_TROOP_LEADER_TARGET_PLAYER", 41: "CON_PLAYER_ATTACKED",
    42: "CON_DAM_OPENED", 43: "CON_PLAYER_IN_SIGHT", 44: "CON_RANGE_TROOP_IN_AREA",
    45: "CON_PLAYER_CLOSE_TO_TROOP", 46: "CON_PLAYER_CLOSE_TO_PLAYER",
    47: "CON_PLAYER_MELEE_ATTACKED", 48: "CON_PLAYER_WITH_THE_SUN_IN_BACK",
    49: "CON_PLAYER_ATTACKED_TROOP", 50: "CON_PLAYER_NOT_ENGAGED",
    51: "CON_TROOP_UNBLOCKABLE_ATTACKED", 52: "CON_TROOP_ABILITY_ATTACKED",
    53: "CON_IS_WATER_FLOODED_IN_AREA", 54: "CON_SP", 55: "CON_TROOP_SCALE",
    56: "CON_TROOP_ATTACKED_PLAYER", 57: "CON_TROOP_ATTACKED_BY_FLOOD",
    58: "CON_TROOP_ATTACK_WITH_FACING_THE_SUN", 59: "CON_PLAYER_SCOUTER_IS_NOT_IN_AREA",
    60: "CON_SELECTED_TROOP",
}

ACT_NAMES = {
    0: "ACT_TRIGGER_ACTIVATE", 1: "ACT_TRIGGER_DEACTIVATE",
    2: "ACT_MARK_ON_TIME", 3: "ACT_POINT_SHOW_IN_MINIMAP",
    4: "ACT_POINT_HIDE_IN_MINIMAP", 5: "ACT_TROOP_INDICATE_IN_MINIMAP",
    6: "ACT_CHAR_SAY", 7: "ACT_TROOP_SET_PARAM",
    8: "ACT_TROOP_ENABLE", 9: "ACT_TROOP_DISABLE",
    10: "ACT_TROOP_WALK_TO", 11: "ACT_TROOP_RUN_TO",
    12: "ACT_TROOP_ADD_WAYPOINT", 13: "ACT_TROOP_FOLLOW",
    14: "ACT_TROOP_STOP", 15: "ACT_CAM_SET", 16: "ACT_CAM_FORCE",
    17: "ACT_TROOP_RETREAT_TO", 18: "ACT_TROOP_ATTACK",
    19: "ACT_TROOP_SET_TRAP", 20: "ACT_TROOP_MORALE_UP",
    21: "ACT_SET_CURSOR_POS", 22: "ACT_RESET_ALL_TRIGGERS",
    23: "ACT_ADD_SP", 24: "ACT_RIVER_FLOODED",
    26: "ACT_TROOP_ABILITY", 27: "ACT_TROOP_ABILITY_TO_TROOP",
    28: "ACT_TROOP_ATTACK_LEADER", 29: "ACT_HIDE_VAR",
    32: "ACT_VAR_INCREASE", 33: "ACT_VAR_DISPLAY",
    34: "ACT_SHOW_SKIPPING_MESSAGE", 35: "ACT_TROOP_ANNIHILATED",
    38: "ACT_OPEN_SESAME", 39: "ACT_CLOSE_SESAME",
    47: "ACT_EVENT_ANCIENT_HEART_CALLED_ME", 49: "ACT_MISSION_COMPLETE",
    50: "ACT_MISSION_FAIL", 51: "ACT_DELAY_TICK",
    52: "ACT_LOOP", 53: "ACT_RESET_TRIGGER",
    54: "ACT_SHOW_TEXT", 55: "ACT_VAR_INT_SET",
    56: "ACT_VAR_RANDOM_SET", 57: "ACT_CAM_RESET",
    58: "ACT_LETTER_BOX_ENABLE", 59: "ACT_LETTER_BOX_DISABLE",
    60: "ACT_SHOW_TEXT_EX", 61: "ACT_RESET_TRIGGER_EX",
    62: "ACT_TROOP_SIGNAL", 63: "ACT_BLOCK_AREA",
    64: "ACT_OPEN_AREA", 65: "ACT_RECOVER_AREA",
    66: "ACT_SET_AI", 67: "ACT_ENABLE_AI", 68: "ACT_DISABLE_AI",
    70: "ACT_SHOW_TEXT_XY_2", 71: "ACT_SET_SNOW",
    72: "ACT_REMOVE_SNOW", 73: "ACT_SET_CAM_TARGET",
    74: "ACT_UNSET_CAM_TARGET", 75: "ACT_RENEW_TROOP",
    76: "ACT_SHOW_TITLE", 77: "ACT_SHOW_TEXT_XY",
    78: "ACT_SET_FPS", 79: "ACT_RESET_FPS",
    80: "ACT_SET_MOTION_BLUR", 81: "ACT_RESET_MOTION_BLUR",
    82: "ACT_TROOP_SET_SPEED", 83: "ACT_TROOP_RESET_SPEED",
    84: "ACT_SET_RAIN", 85: "ACT_STOP_RAIN",
    86: "ACT_SET_WIND", 87: "ACT_SET_GATE",
    88: "ACT_START_WATER_ATTACK", 89: "ACT_CHAR_SAY_EX",
    90: "ACT_LEADER_INVULNERABLE", 91: "ACT_LEADER_VULNERABLE",
    92: "ACT_LEADER_RECHARE_RATE", 93: "ACT_TROOP_SET_BOUNDARY",
    94: "ACT_TROOP_RESET_BOUNDARY", 95: "ACT_TROOP_WARP",
    96: "ACT_MY_PLAYER_GET_EXP", 97: "ACT_SHOW_AREA_ON_MINIMAP",
    98: "ACT_HIDE_AREA_ON_MINIMAP", 99: "ACT_SHOW_TEXT_ON_MSG_WINDOW",
    100: "ACT_TROOP_SIGNAL_ARROW", 101: "ACT_FADE_IN",
    102: "ACT_FADE_OUT", 103: "ACT_OPEN_DAM", 104: "ACT_CLOSE_DAM",
    105: "ACT_DISABLE_TROOPS_INSIDE_AREA", 106: "ACT_DISABLE_TROOPS_OUTSIDE_AREA",
    107: "ACT_DISABLE_ALL_TROOPS", 109: "ACT_COLLAPSE_WALL",
    110: "ACT_PLAY_BGM", 111: "ACT_STOP_BGM",
    112: "ACT_START_TROOP_INDICATE_IN_MINIMAP",
    113: "ACT_STOP_TROOP_INDICATE_IN_MINIMAP",
    114: "ACT_TROOP_REFILL_HP", 115: "ACT_TROOP_SET_HP",
    116: "ACT_LIP_SYNC_BEGIN", 117: "ACT_LIP_SYNC_END",
    118: "ACT_SET_FIRE_SPREAD_SPEED", 119: "ACT_ENABLE_INPUT",
    120: "ACT_DISABLE_INPUT", 121: "ACT_ENABLE_FOG_OF_WAR",
    122: "ACT_SET_FIRE_SPREAD_RANGE", 123: "ACT_SET_FIRE",
    124: "ACT_REGNIER_GO_CRAZY", 125: "ACT_REGNIER_FREE_HIS_POWER",
    126: "ACT_SET_MUTE", 127: "ACT_BURY_TROOP",
    129: "ACT_TAG_THE_TROOP", 130: "ACT_UNTAG_THE_TROOP",
    131: "ACT_SHOW_VAR_GAUGE", 132: "ACT_HIDE_VAR_GAUGE",
    133: "ACT_TROOP_ANIMATION", 135: "ACT_TROOP_RANGE_ATTACK_ON_POS",
    136: "ACT_TROOP_RANGE_ATTACK_ON_PROP", 137: "ACT_SET_AI_PATH",
    138: "ACT_ENABLE_ABILITY", 139: "ACT_DISABLE_ABILITY",
    140: "ACT_SHOW_NOISE_METER_GAUGE", 141: "ACT_SET_BGM_VOLUME",
    142: "ACT_FADE_BGM", 143: "ACT_MARK_ON_TROOP_IN_AREA",
    144: "ACT_SET_WALL_HP", 145: "ACT_TROOP_SET_INVULNERABLE",
    146: "ACT_TROOP_RESET_INVULNERABLE", 147: "ACT_TROOP_SELECT",
    148: "ACT_FORCE_MINIMAP_ON", 149: "ACT_FORCE_MINIMAP_OFF",
    152: "ACT_RENEW_TROOP_OUTOFSIGHT", 153: "ACT_SET_TRAP",
    154: "ACT_SET_MINE", 155: "ACT_FLOOD_RESET",
    157: "ACT_ENABLE_PAUSE", 158: "ACT_DISABLE_PAUSE",
    159: "ACT_QUICK_SAVE", 161: "ACT_ENABLE_TROOP_IN_AREA",
    162: "ACT_PLAYER_TROOP_STOP", 163: "ACT_EXCLUSIVE_BEGIN",
    164: "ACT_EXCLUSIVE_END", 165: "ACT_LOAD_MISSION",
    166: "ACT_JOYPAD_RUMBLE", 168: "ACT_SET_FIRE_N_SMOKE",
    169: "ACT_SET_WATER_EFFECT_PROP", 170: "ACT_SET_SCREEN_GLOW",
    171: "ACT_UPDATE_UNIT_KILL_COUNT", 172: "ACT_PLAY_FMV",
    173: "ACT_ENABLE_LENS_FLARE", 174: "ACT_CHANGE_SKYBOX_N_LIGHT_SET",
    175: "ACT_REMOVE_TRAP", 176: "ACT_SET_FIRE_N_SMOKE_SMALL",
    177: "ACT_SET_TRAINING_MISSION", 178: "ACT_SET_LIBRARY",
    179: "ACT_ALL_MISSION_COMPLETE", 180: "ACT_PLAY_FMV_AND_GO_TO_WORLDMAP",
    181: "ACT_GO_TO_WORLDMAP", 182: "ACT_SKIP_TEXT",
}


def parse_variable_section(data, offset, file_size):
    """Parse the variable section and return (variable_count, section_end_offset)."""
    if offset + 4 > file_size:
        return 0, offset
    count = u32(data, offset)
    pos = offset + 4
    for i in range(count):
        name_start = pos
        while pos < file_size and data[pos] != 0:
            pos += 1
        if pos >= file_size:
            return count, pos
        name = data[name_start:pos].decode('cp949', errors='replace')
        pos += 1  # skip null
        if pos + 12 > file_size:
            return count, pos
        var_id = u32(data, pos)
        pad = u32(data, pos + 4)
        init_val = u32(data, pos + 8)
        print(f"  var[{i}]: name='{name}' id={var_id} pad={pad} init={init_val}")
        pos += 12
    return count, pos


def analyze_file(path):
    data = path.read_bytes()
    file_size = len(data)
    print(f"\n{'#'*70}")
    print(f"# {path.name} ({file_size} bytes)")
    print(f"{'#'*70}")

    # Header
    unit_count = u32(data, 0x270)
    tail_offset = 628 + unit_count * 544
    print(f"  units={unit_count}, tail starts at 0x{tail_offset:X}")

    # AreaIDs
    area_count = u32(data, tail_offset)
    area_section_size = 4 + area_count * 84
    print(f"  area_count={area_count}, area_section={area_section_size} bytes")

    # Variables
    var_offset = tail_offset + area_section_size
    print(f"  Variables at 0x{var_offset:X}:")
    var_count, after_vars = parse_variable_section(data, var_offset, file_size)
    print(f"  var_count={var_count}, after_vars=0x{after_vars:X}")

    # What's between vars and event count?
    # Previous analysis found 2 mystery u32s (1, 0) here.
    # But let's just dump from after_vars to see what's there.
    remaining = file_size - after_vars
    print(f"\n  Remaining after vars: {remaining} bytes")
    print(f"  Dumping first 32 bytes after vars:")
    hexdump(data, after_vars, min(32, remaining), "    ")

    # Try: the next u32 might be the event count directly.
    # Or there may be 8 mystery bytes first.
    # Let's try both interpretations.

    for label, event_count_offset in [
        ("Direct (event count at after_vars)", after_vars),
        ("Skip 8 bytes (mystery 1,0)", after_vars + 8),
    ]:
        print(f"\n  === {label} ===")
        if event_count_offset + 4 > file_size:
            print(f"    Out of bounds")
            continue

        event_count = u32(data, event_count_offset)
        events_start = event_count_offset + 4
        footer_space = file_size - events_start - 42  # reserve 42 for footer

        print(f"    event_count = {event_count}")
        print(f"    events_start = 0x{events_start:X}")
        print(f"    space for events (minus 42 footer) = {footer_space} bytes")

        if event_count > 200 or event_count == 0:
            print(f"    Skipping (event count unreasonable)")
            continue

        # Now try to parse events as variable-length
        pos = events_start
        success_count = 0
        for ev in range(min(event_count, 50)):
            if pos + 72 > file_size:
                print(f"    Event {ev}: OUT OF BOUNDS at 0x{pos:X}")
                break

            # Description (64 bytes)
            desc_raw = data[pos:pos+64]
            desc_end = desc_raw.index(0) if 0 in desc_raw else 64
            try:
                desc = desc_raw[:desc_end].decode('cp949', errors='replace')
            except:
                desc = desc_raw[:desc_end].decode('ascii', errors='replace')

            block_id = u32(data, pos + 64)
            num_cond = u32(data, pos + 68)

            print(f"\n    Event {ev} at 0x{pos:X}: desc='{desc}' blockId={block_id} numCond={num_cond}")

            if num_cond > 50:
                print(f"      INVALID numCond, stopping")
                break

            p = pos + 72
            cond_ok = True
            for c in range(num_cond):
                if p + 4 > file_size:
                    print(f"      cond[{c}]: OUT OF BOUNDS")
                    cond_ok = False
                    break
                cid = u32(data, p)
                pc = COND_PARAMS.get(cid)
                cname = COND_NAMES.get(cid, f"CON_{cid}")
                if pc is None:
                    print(f"      cond[{c}] at 0x{p:X}: id={cid} ({cname}) UNKNOWN!")
                    hexdump(data, p, min(32, file_size - p), "        ")
                    cond_ok = False
                    break
                if p + 4 + pc * 4 > file_size:
                    print(f"      cond[{c}]: params out of bounds")
                    cond_ok = False
                    break
                params = [u32(data, p + 4 + i*4) for i in range(pc)]
                entry_size = 4 + pc * 4
                print(f"      cond[{c}] at 0x{p:X}: {cname}({cid}) params={params} [{entry_size}B]")
                p += entry_size

            if not cond_ok:
                break

            # numActions
            if p + 4 > file_size:
                print(f"      numActions: OUT OF BOUNDS")
                break
            num_act = u32(data, p)
            print(f"      numActions at 0x{p:X}: {num_act}")
            p += 4

            if num_act > 100:
                print(f"      INVALID numAct, stopping")
                break

            act_ok = True
            for a in range(num_act):
                if p + 4 > file_size:
                    print(f"      act[{a}]: OUT OF BOUNDS")
                    act_ok = False
                    break
                aid = u32(data, p)
                pc = ACT_PARAMS.get(aid)
                aname = ACT_NAMES.get(aid, f"ACT_{aid}")

                if pc is None:
                    print(f"      act[{a}] at 0x{p:X}: id={aid} ({aname}) UNKNOWN!")
                    hexdump(data, p, min(32, file_size - p), "        ")
                    act_ok = False
                    break

                if pc == -1:
                    # String or variable-length action - try to handle
                    print(f"      act[{a}] at 0x{p:X}: {aname}({aid}) STRING/VARLEN action")
                    # For string actions, we need to figure out the format.
                    # Let's dump context and skip.
                    hexdump(data, p, min(64, file_size - p), "        ")
                    act_ok = False
                    break

                if p + 4 + pc * 4 > file_size:
                    print(f"      act[{a}]: params out of bounds")
                    act_ok = False
                    break

                params = [u32(data, p + 4 + i*4) for i in range(pc)]
                entry_size = 4 + pc * 4
                print(f"      act[{a}] at 0x{p:X}: {aname}({aid}) params={params} [{entry_size}B]")
                p += entry_size

            if not act_ok:
                break

            event_size = p - pos
            print(f"      Event {ev} total size: {event_size} bytes, next at 0x{p:X}")
            success_count += 1

        print(f"\n    Parsed {success_count}/{event_count} events")
        if success_count > 0 and success_count == event_count:
            print(f"    ALL EVENTS PARSED SUCCESSFULLY!")
            print(f"    After events: 0x{p:X}")
            remaining_after = file_size - p
            print(f"    Remaining bytes: {remaining_after}")
            print(f"    Footer dump:")
            hexdump(data, p, remaining_after, "      ")


def main():
    mission_dir = Path.home() / "Downloads" / "KUF Crusaders" / "Mission"

    # Start with E1140 (simplest: 1 unit, 2 events)
    for name in ["E1140.stg", "TUTO_D1.stg", "E1001.stg"]:
        path = mission_dir / name
        if path.exists():
            analyze_file(path)


if __name__ == '__main__':
    main()
