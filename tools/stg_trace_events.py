#!/usr/bin/env python3
"""Trace event parsing for TUTO_D1.stg and E1001.stg with detailed output."""

import struct
from pathlib import Path

def u32(data, off):
    return struct.unpack_from('<I', data, off)[0]

def hexdump(data, offset, length, prefix=""):
    for i in range(0, length, 16):
        off = offset + i
        n = min(16, length - i, len(data) - off)
        if n <= 0: break
        hx = ' '.join(f'{data[off+j]:02X}' for j in range(n))
        print(f'{prefix}0x{off:04X}: {hx}')


# From MISSION_SCRIPTING.md - CAREFULLY DERIVED param counts.
# Format: id: (param_count, name)
# "int seconds, compare" = 2 params
# "TroopID, int%, compare" = 3 params
# etc.
COND_INFO = {
    0: (2, "CON_TIME_ELAPSED"),          # int seconds, compare
    1: (3, "CON_TIME_ELAPSED_FROM_MARKED"), # TimeMarkID, int seconds, compare
    2: (2, "CON_TROOP_IN_AREA"),         # TroopID, AreaID
    3: (2, "CON_TROOP_SCOUTER_STOPPED_IN_AREA"), # TroopID, AreaID
    4: (2, "CON_TROOP_SCOUTER_IN_AREA"), # TroopID, AreaID
    5: (3, "CON_TROOP_SCOUTER_CLOSE_TO_TROOP"), # TroopID, TroopID, float
    6: (4, "CON_TROOP_CLOSE_TO_TROOP"),  # TroopID, TroopID, float, compare
    7: (2, "CON_TROOP_TARGETED"),        # TroopID, PlayerID
    8: (1, "CON_TROOP_ATTACKED"),        # TroopID
    9: (1, "CON_TROOP_MELEE_ATTACKED"),  # TroopID
    10: (2, "CON_TROOP_ABILITY"),        # TroopID, Ability
    11: (2, "CON_TROOP_WITH_THE_SUN_IN_BACK"), # TroopID, int
    12: (1, "CON_LEADER_HAS_BEEN_KILLED"), # TroopID
    13: (3, "CON_STATE_HP_PERCENT"),     # TroopID, int%, compare
    14: (3, "CON_LEADER_HP"),            # TroopID, int%, compare
    15: (2, "CON_OBJECT_LOCATED"),       # AreaID, ObjectType
    17: (3, "CON_OBJECT_HP_PERCENT"),    # ObjectID, int%, compare
    18: (1, "CON_OBJECT_DETECTED"),      # ObjectType
    19: (3, "CON_VAR"),                  # ValueID, int, compare
    20: (2, "CON_PLAYER_TROOP_ALL_DISABLED"), # PlayerID, int%
    22: (2, "CON_PLAYER_ALL_IN_AREA"),   # PlayerID, AreaID
    23: (2, "CON_TROOP_NOT_IN_AREA"),    # TroopID, AreaID
    24: (2, "CON_TROOP_ATTACKED_TROOP"), # TroopID, TargetTroopID
    25: (2, "CON_PLAYER_TROOP_NOT_IN_AREA"), # PlayerID, AreaID
    26: (2, "CON_PLAYER_TROOP_IN_AREA"), # PlayerID, AreaID
    27: (0, "CON_ALWAYS_TRUE"),          # -
    28: (1, "CON_PLAYER_KO"),            # PlayerID
    29: (1, "CON_SCOUTER_BACK"),         # TroopID
    30: (1, "CON_TROOP_IN_SIGHT"),       # TroopID
    31: (2, "CON_TROOP_TYPE"),           # TroopID, TroopType
    32: (3, "CON_TROOP_TYPE_IN_AREA"),   # PlayerID, TroopType, AreaID
    33: (1, "CON_TROOP_NOT_ENGAGED"),    # TroopID
    34: (3, "CON_PLAYER_HP_SUM"),        # PlayerID, int, compare
    35: (1, "CON_GOT_FIRE"),             # AreaID
    36: (1, "CON_CAM_DIR"),              # AreaID
    37: (0, "CON_IS_DEMO_SKIPPED"),      # -
    38: (2, "CON_IS_MINE_AT"),           # PlayerGroupID, AreaID
    39: (2, "CON_IS_TRAP_AT"),           # PlayerGroupID, AreaID
    40: (1, "CON_TROOP_LEADER_TARGET_PLAYER"), # TroopID
    41: (1, "CON_PLAYER_ATTACKED"),      # PlayerID
    42: (0, "CON_DAM_OPENED"),           # -
    43: (1, "CON_PLAYER_IN_SIGHT"),      # PlayerID
    44: (2, "CON_RANGE_TROOP_IN_AREA"),  # AreaID, PlayerID
    45: (4, "CON_PLAYER_CLOSE_TO_TROOP"), # PlayerID, TroopID, float, compare
    46: (4, "CON_PLAYER_CLOSE_TO_PLAYER"), # PlayerID, PlayerID, float, compare
    47: (1, "CON_PLAYER_MELEE_ATTACKED"), # PlayerID
    48: (1, "CON_PLAYER_WITH_THE_SUN_IN_BACK"), # PlayerID
    49: (2, "CON_PLAYER_ATTACKED_TROOP"), # PlayerID, TroopID
    50: (1, "CON_PLAYER_NOT_ENGAGED"),   # PlayerID
    51: (1, "CON_TROOP_UNBLOCKABLE_ATTACKED"), # TroopID
    52: (2, "CON_TROOP_ABILITY_ATTACKED"), # TroopID, AbilityID
    53: (1, "CON_IS_WATER_FLOODED_IN_AREA"), # AreaID
    54: (3, "CON_SP"),                   # PlayerID, int, compare
    55: (2, "CON_TROOP_SCALE"),          # TroopID, int
    56: (2, "CON_TROOP_ATTACKED_PLAYER"), # TroopID, PlayerID
    57: (1, "CON_TROOP_ATTACKED_BY_FLOOD"), # TroopID
    58: (1, "CON_TROOP_ATTACK_WITH_FACING_THE_SUN"), # TroopID
    59: (2, "CON_PLAYER_SCOUTER_IS_NOT_IN_AREA"), # PlayerID, AreaID
    60: (1, "CON_SELECTED_TROOP"),       # TroopID
}

ACT_INFO = {
    0: (1, "ACT_TRIGGER_ACTIVATE"),
    1: (1, "ACT_TRIGGER_DEACTIVATE"),
    2: (1, "ACT_MARK_ON_TIME"),
    3: (1, "ACT_POINT_SHOW_IN_MINIMAP"),
    4: (1, "ACT_POINT_HIDE_IN_MINIMAP"),
    5: (1, "ACT_TROOP_INDICATE_IN_MINIMAP"),
    6: (2, "ACT_CHAR_SAY"),
    7: (3, "ACT_TROOP_SET_PARAM"),
    8: (1, "ACT_TROOP_ENABLE"),
    9: (1, "ACT_TROOP_DISABLE"),
    10: (2, "ACT_TROOP_WALK_TO"),
    11: (2, "ACT_TROOP_RUN_TO"),
    12: (3, "ACT_TROOP_ADD_WAYPOINT"),
    13: (3, "ACT_TROOP_FOLLOW"),
    14: (1, "ACT_TROOP_STOP"),
    15: (2, "ACT_CAM_SET"),
    16: (2, "ACT_CAM_FORCE"),
    17: (2, "ACT_TROOP_RETREAT_TO"),
    18: (2, "ACT_TROOP_ATTACK"),
    19: (2, "ACT_TROOP_SET_TRAP"),
    20: (1, "ACT_TROOP_MORALE_UP"),
    21: (1, "ACT_SET_CURSOR_POS"),
    22: (0, "ACT_RESET_ALL_TRIGGERS"),
    23: (2, "ACT_ADD_SP"),
    24: (0, "ACT_RIVER_FLOODED"),
    26: (3, "ACT_TROOP_ABILITY"),
    27: (3, "ACT_TROOP_ABILITY_TO_TROOP"),
    28: (2, "ACT_TROOP_ATTACK_LEADER"),
    29: (0, "ACT_HIDE_VAR"),
    32: (2, "ACT_VAR_INCREASE"),
    33: (4, "ACT_VAR_DISPLAY"),  # VarID, int, int, string -> treat string separately
    34: (1, "ACT_SHOW_SKIPPING_MESSAGE"),
    35: (1, "ACT_TROOP_ANNIHILATED"),
    38: (1, "ACT_OPEN_SESAME"),
    39: (1, "ACT_CLOSE_SESAME"),
    47: (0, "ACT_EVENT_ANCIENT_HEART"),
    49: (0, "ACT_MISSION_COMPLETE"),
    50: (0, "ACT_MISSION_FAIL"),
    51: (1, "ACT_DELAY_TICK"),
    52: (0, "ACT_LOOP"),
    53: (0, "ACT_RESET_TRIGGER"),
    54: (1, "ACT_SHOW_TEXT"),
    55: (2, "ACT_VAR_INT_SET"),
    56: (2, "ACT_VAR_RANDOM_SET"),
    57: (0, "ACT_CAM_RESET"),
    58: (0, "ACT_LETTER_BOX_ENABLE"),
    59: (0, "ACT_LETTER_BOX_DISABLE"),
    60: (2, "ACT_SHOW_TEXT_EX"),
    61: (1, "ACT_RESET_TRIGGER_EX"),
    62: (2, "ACT_TROOP_SIGNAL"),
    63: (1, "ACT_BLOCK_AREA"),
    64: (1, "ACT_OPEN_AREA"),
    65: (1, "ACT_RECOVER_AREA"),
    66: (2, "ACT_SET_AI"),
    67: (1, "ACT_ENABLE_AI"),
    68: (1, "ACT_DISABLE_AI"),
    70: (4, "ACT_SHOW_TEXT_XY_2"),
    71: (1, "ACT_SET_SNOW"),
    72: (0, "ACT_REMOVE_SNOW"),
    73: (1, "ACT_SET_CAM_TARGET"),
    74: (0, "ACT_UNSET_CAM_TARGET"),
    75: (1, "ACT_RENEW_TROOP"),
    76: (3, "ACT_SHOW_TITLE"),
    77: (4, "ACT_SHOW_TEXT_XY"),
    78: (1, "ACT_SET_FPS"),
    79: (0, "ACT_RESET_FPS"),
    80: (1, "ACT_SET_MOTION_BLUR"),
    81: (0, "ACT_RESET_MOTION_BLUR"),
    82: (2, "ACT_TROOP_SET_SPEED"),
    83: (1, "ACT_TROOP_RESET_SPEED"),
    84: (1, "ACT_SET_RAIN"),
    85: (0, "ACT_STOP_RAIN"),
    86: (2, "ACT_SET_WIND"),
    87: (3, "ACT_SET_GATE"),
    88: (0, "ACT_START_WATER_ATTACK"),
    89: (3, "ACT_CHAR_SAY_EX"),
    90: (1, "ACT_LEADER_INVULNERABLE"),
    91: (1, "ACT_LEADER_VULNERABLE"),
    92: (1, "ACT_LEADER_RECHARGE_RATE"),
    93: (2, "ACT_TROOP_SET_BOUNDARY"),
    94: (1, "ACT_TROOP_RESET_BOUNDARY"),
    95: (3, "ACT_TROOP_WARP"),
    96: (1, "ACT_MY_PLAYER_GET_EXP"),
    97: (1, "ACT_SHOW_AREA_ON_MINIMAP"),
    98: (1, "ACT_HIDE_AREA_ON_MINIMAP"),
    99: (3, "ACT_SHOW_TEXT_ON_MSG_WINDOW"),
    100: (2, "ACT_TROOP_SIGNAL_ARROW"),
    101: (1, "ACT_FADE_IN"),
    102: (1, "ACT_FADE_OUT"),
    103: (0, "ACT_OPEN_DAM"),
    104: (0, "ACT_CLOSE_DAM"),
    105: (2, "ACT_DISABLE_TROOPS_INSIDE_AREA"),
    106: (2, "ACT_DISABLE_TROOPS_OUTSIDE_AREA"),
    107: (0, "ACT_DISABLE_ALL_TROOPS"),
    109: (1, "ACT_COLLAPSE_WALL"),
    111: (1, "ACT_STOP_BGM"),
    112: (1, "ACT_START_TROOP_INDICATE_IN_MINIMAP"),
    113: (1, "ACT_STOP_TROOP_INDICATE_IN_MINIMAP"),
    114: (1, "ACT_TROOP_REFILL_HP"),
    115: (2, "ACT_TROOP_SET_HP"),
    116: (2, "ACT_LIP_SYNC_BEGIN"),
    117: (2, "ACT_LIP_SYNC_END"),
    118: (1, "ACT_SET_FIRE_SPREAD_SPEED"),
    119: (0, "ACT_ENABLE_INPUT"),
    120: (0, "ACT_DISABLE_INPUT"),
    121: (1, "ACT_ENABLE_FOG_OF_WAR"),
    122: (1, "ACT_SET_FIRE_SPREAD_RANGE"),
    123: (1, "ACT_SET_FIRE"),
    124: (1, "ACT_REGNIER_GO_CRAZY"),
    125: (1, "ACT_REGNIER_FREE_HIS_POWER"),
    126: (1, "ACT_SET_MUTE"),
    127: (1, "ACT_BURY_TROOP"),
    129: (2, "ACT_TAG_THE_TROOP"),
    130: (1, "ACT_UNTAG_THE_TROOP"),
    131: (3, "ACT_SHOW_VAR_GAUGE"),
    132: (0, "ACT_HIDE_VAR_GAUGE"),
    133: (2, "ACT_TROOP_ANIMATION"),
    135: (2, "ACT_TROOP_RANGE_ATTACK_ON_POS"),
    136: (2, "ACT_TROOP_RANGE_ATTACK_ON_PROP"),
    138: (1, "ACT_ENABLE_ABILITY"),
    139: (1, "ACT_DISABLE_ABILITY"),
    140: (1, "ACT_SHOW_NOISE_METER_GAUGE"),
    141: (2, "ACT_SET_BGM_VOLUME"),
    142: (3, "ACT_FADE_BGM"),
    143: (3, "ACT_MARK_ON_TROOP_IN_AREA"),
    144: (2, "ACT_SET_WALL_HP"),
    145: (1, "ACT_TROOP_SET_INVULNERABLE"),
    146: (1, "ACT_TROOP_RESET_INVULNERABLE"),
    147: (1, "ACT_TROOP_SELECT"),
    148: (0, "ACT_FORCE_MINIMAP_ON"),
    149: (0, "ACT_FORCE_MINIMAP_OFF"),
    152: (4, "ACT_RENEW_TROOP_OUTOFSIGHT"),
    153: (3, "ACT_SET_TRAP"),
    154: (3, "ACT_SET_MINE"),
    155: (0, "ACT_FLOOD_RESET"),
    157: (0, "ACT_ENABLE_PAUSE"),
    158: (0, "ACT_DISABLE_PAUSE"),
    159: (0, "ACT_QUICK_SAVE"),
    161: (2, "ACT_ENABLE_TROOP_IN_AREA"),
    162: (1, "ACT_PLAYER_TROOP_STOP"),
    163: (0, "ACT_EXCLUSIVE_BEGIN"),
    164: (0, "ACT_EXCLUSIVE_END"),
    166: (1, "ACT_JOYPAD_RUMBLE"),
    168: (2, "ACT_SET_FIRE_N_SMOKE"),
    169: (1, "ACT_SET_WATER_EFFECT_PROP"),
    170: (1, "ACT_SET_SCREEN_GLOW"),
    171: (2, "ACT_UPDATE_UNIT_KILL_COUNT"),
    173: (1, "ACT_ENABLE_LENS_FLARE"),
    175: (2, "ACT_REMOVE_TRAP"),
    176: (2, "ACT_SET_FIRE_N_SMOKE_SMALL"),
    177: (2, "ACT_SET_TRAINING_MISSION"),
    178: (2, "ACT_SET_LIBRARY"),
    179: (0, "ACT_ALL_MISSION_COMPLETE"),
    181: (0, "ACT_GO_TO_WORLDMAP"),
    182: (0, "ACT_SKIP_TEXT"),
    300: (1, "ACT_LIVE_SET_RENEW_AREA"),
}

# String actions that need special handling
STRING_ACTS = {110, 137, 150, 151, 160, 165, 167, 172, 174, 180, 709, 729}


def trace_events(data, events_start, event_count, footer_start, label):
    """Parse and trace events with full detail."""
    print(f"\n{'='*70}")
    print(f"# {label}")
    print(f"# events_start=0x{events_start:X}, count={event_count}, "
          f"footer=0x{footer_start:X}")
    print(f"# Event data: {footer_start - events_start} bytes")
    print(f"{'='*70}")

    pos = events_start
    for ev in range(event_count):
        if pos + 72 > footer_start:
            print(f"\n  Event {ev}: OUT OF BOUNDS at 0x{pos:X}")
            break

        desc = data[pos:pos+64]
        desc_end = desc.index(0) if 0 in desc else 64
        try:
            desc_str = desc[:desc_end].decode('cp949', errors='replace')
        except:
            desc_str = desc[:desc_end].decode('ascii', errors='replace')

        block_id = u32(data, pos + 64)
        num_cond = u32(data, pos + 68)

        print(f"\n  Event {ev} at 0x{pos:X}: desc='{desc_str}' blockId={block_id} "
              f"numCond={num_cond}")

        if num_cond > 50:
            print(f"    INVALID numCond! Hex context:")
            hexdump(data, pos + 64, 32, "      ")
            break

        p = pos + 72
        cond_ok = True
        for c in range(num_cond):
            if p + 4 > footer_start:
                print(f"    cond[{c}]: OOB at 0x{p:X}")
                cond_ok = False
                break
            cid = u32(data, p)
            info = COND_INFO.get(cid)
            if info is None:
                print(f"    cond[{c}] at 0x{p:X}: UNKNOWN id={cid} (0x{cid:X})")
                hexdump(data, p, min(32, footer_start - p), "      ")
                cond_ok = False
                break
            pc, name = info
            if p + 4 + pc*4 > footer_start:
                print(f"    cond[{c}] {name}: params OOB")
                cond_ok = False
                break
            params = [u32(data, p + 4 + i*4) for i in range(pc)]
            print(f"    cond[{c}] at 0x{p:X}: {name}({cid}) params={params} "
                  f"[{4+pc*4}B]")
            p += 4 + pc * 4

        if not cond_ok:
            break

        if p + 4 > footer_start:
            print(f"    numAct OOB")
            break
        num_act = u32(data, p)
        print(f"    numAct at 0x{p:X}: {num_act}")
        p += 4

        if num_act > 100:
            print(f"    INVALID numAct!")
            hexdump(data, p - 8, 48, "      ")
            break

        act_ok = True
        for a in range(num_act):
            if p + 4 > footer_start:
                print(f"    act[{a}]: OOB at 0x{p:X}")
                act_ok = False
                break
            aid = u32(data, p)

            if aid in STRING_ACTS:
                print(f"    act[{a}] at 0x{p:X}: STRING ACTION id={aid}")
                hexdump(data, p, min(64, footer_start - p), "      ")
                act_ok = False
                break

            info = ACT_INFO.get(aid)
            if info is None:
                print(f"    act[{a}] at 0x{p:X}: UNKNOWN id={aid} (0x{aid:X})")
                hexdump(data, p, min(32, footer_start - p), "      ")
                act_ok = False
                break
            pc, name = info
            if p + 4 + pc*4 > footer_start:
                print(f"    act[{a}] {name}: params OOB")
                act_ok = False
                break
            params = [u32(data, p + 4 + i*4) for i in range(pc)]
            print(f"    act[{a}] at 0x{p:X}: {name}({aid}) params={params} "
                  f"[{4+pc*4}B]")
            p += 4 + pc * 4

        if not act_ok:
            break

        ev_size = p - pos
        print(f"    Event {ev} total: {ev_size} bytes, next at 0x{p:X}")
        pos = p

    remaining = footer_start - pos
    print(f"\n  Stopped at 0x{pos:X}, remaining={remaining} bytes")
    if remaining > 0 and remaining < 64:
        print(f"  Footer candidate:")
        hexdump(data, pos, remaining, "    ")


def main():
    mission_dir = Path.home() / "Downloads" / "KUF Crusaders" / "Mission"

    for name, ec_offset in [("TUTO_D1.stg", 0x1580), ("E1001.stg", 0x7390)]:
        path = mission_dir / name
        if not path.exists():
            continue
        data = path.read_bytes()
        file_size = len(data)
        ec = u32(data, ec_offset)
        ev_start = ec_offset + 4
        footer_start = file_size - 42
        trace_events(data, ev_start, ec, footer_start, name)


if __name__ == '__main__':
    main()
