#!/usr/bin/env python3
"""Analyze STG event format - v2 with correct CruMission.h IDs."""

import struct
import sys
from pathlib import Path

def read_u32(data, offset):
    return struct.unpack_from('<I', data, offset)[0]

def read_f32(data, offset):
    return struct.unpack_from('<f', data, offset)[0]

def read_cstr(data, offset, maxlen):
    end = offset + maxlen
    for i in range(offset, end):
        if data[i] == 0:
            end = i
            break
    return data[offset:end].decode('ascii', errors='replace')

def hexdump(data, offset, length, prefix="    "):
    for i in range(0, length, 16):
        off = offset + i
        if off + 16 > len(data):
            nbytes = len(data) - off
        else:
            nbytes = min(16, length - i)
        if nbytes <= 0:
            break
        hexb = ' '.join(f'{data[off+j]:02X}' for j in range(nbytes))
        asc = ''.join(chr(data[off+j]) if 32 <= data[off+j] < 127 else '.' for j in range(nbytes))
        print(f'{prefix}0x{off:04X}: {hexb:<48} {asc}')

# Condition param counts from MISSION_SCRIPTING.md (CruMission.h)
COND_PARAMS = {
    0: 2,    # CON_TIME_ELAPSED: seconds, compare
    1: 3,    # CON_TIME_ELAPSED_FROM_MARKED: TimeMarkID, seconds, compare
    2: 2,    # CON_TROOP_IN_AREA: TroopID, AreaID
    3: 2,    # CON_TROOP_SCOUTER_STOPPED_IN_AREA: TroopID, AreaID
    4: 2,    # CON_TROOP_SCOUTER_IN_AREA: TroopID, AreaID
    5: 3,    # CON_TROOP_SCOUTER_CLOSE_TO_TROOP: TroopID, TroopID, float
    6: 4,    # CON_TROOP_CLOSE_TO_TROOP: TroopID, TroopID, float, compare
    7: 2,    # CON_TROOP_TARGETED: TroopID, PlayerID
    8: 1,    # CON_TROOP_ATTACKED: TroopID
    9: 1,    # CON_TROOP_MELEE_ATTACKED: TroopID
    10: 2,   # CON_TROOP_ABILITY: TroopID, Ability
    11: 2,   # CON_TROOP_WITH_THE_SUN_IN_BACK: TroopID, int
    12: 1,   # CON_LEADER_HAS_BEEN_KILLED: TroopID
    13: 3,   # CON_STATE_HP_PERCENT: TroopID, int%, compare
    14: 3,   # CON_LEADER_HP: TroopID, int%, compare
    15: 2,   # CON_OBJECT_LOCATED: AreaID, ObjectType
    17: 3,   # CON_OBJECT_HP_PERCENT: ObjectID, int%, compare
    18: 1,   # CON_OBJECT_DETECTED: ObjectType
    19: 3,   # CON_VAR: ValueID, int, compare
    20: 2,   # CON_PLAYER_TROOP_ALL_DISABLED: PlayerID, int%
    22: 2,   # CON_PLAYER_ALL_IN_AREA: PlayerID, AreaID
    23: 2,   # CON_TROOP_NOT_IN_AREA: TroopID, AreaID
    24: 2,   # CON_TROOP_ATTACKED_TROOP: TroopID, TargetTroopID
    25: 2,   # CON_PLAYER_TROOP_NOT_IN_AREA: PlayerID, AreaID
    26: 2,   # CON_PLAYER_TROOP_IN_AREA: PlayerID, AreaID
    27: 0,   # CON_ALWAYS_TRUE: -
    28: 1,   # CON_PLAYER_KO: PlayerID
    29: 1,   # CON_SCOUTER_BACK: TroopID
    30: 1,   # CON_TROOP_IN_SIGHT: TroopID
    31: 2,   # CON_TROOP_TYPE: TroopID, TroopType
    32: 3,   # CON_TROOP_TYPE_IN_AREA: PlayerID, TroopType, AreaID
    33: 1,   # CON_TROOP_NOT_ENGAGED: TroopID
    34: 3,   # CON_PLAYER_HP_SUM: PlayerID, int, compare
    35: 1,   # CON_GOT_FIRE: AreaID
    36: 1,   # CON_CAM_DIR: AreaID
    37: 0,   # CON_IS_DEMO_SKIPPED: -
    38: 2,   # CON_IS_MINE_AT: PlayerGroupID, AreaID
    39: 2,   # CON_IS_TRAP_AT: PlayerGroupID, AreaID
    40: 1,   # CON_TROOP_LEADER_TARGET_PLAYER: TroopID
    41: 1,   # CON_PLAYER_ATTACKED: PlayerID
    42: 0,   # CON_DAM_OPENED: -
    43: 1,   # CON_PLAYER_IN_SIGHT: PlayerID
    44: 2,   # CON_RANGE_TROOP_IN_AREA: AreaID, PlayerID
    45: 4,   # CON_PLAYER_CLOSE_TO_TROOP: PlayerID, TroopID, float, compare
    46: 4,   # CON_PLAYER_CLOSE_TO_PLAYER: PlayerID, PlayerID, float, compare
    47: 1,   # CON_PLAYER_MELEE_ATTACKED: PlayerID
    48: 1,   # CON_PLAYER_WITH_THE_SUN_IN_BACK: PlayerID
    49: 2,   # CON_PLAYER_ATTACKED_TROOP: PlayerID, TroopID
    50: 1,   # CON_PLAYER_NOT_ENGAGED: PlayerID
    51: 1,   # CON_TROOP_UNBLOCKABLE_ATTACKED: TroopID
    52: 2,   # CON_TROOP_ABILITY_ATTACKED: TroopID, AbilityID
    53: 1,   # CON_IS_WATER_FLOODED_IN_AREA: AreaID
    54: 3,   # CON_SP: PlayerID, int, compare
    55: 2,   # CON_TROOP_SCALE: TroopID, int
    56: 2,   # CON_TROOP_ATTACKED_PLAYER: TroopID, PlayerID
    57: 1,   # CON_TROOP_ATTACKED_BY_FLOOD: TroopID
    58: 1,   # CON_TROOP_ATTACK_WITH_FACING_THE_SUN: TroopID
    59: 2,   # CON_PLAYER_SCOUTER_IS_NOT_IN_AREA: PlayerID, AreaID
    60: 1,   # CON_SELECTED_TROOP: TroopID
    # Worldmap
    300: 1,  # CON_WORLD_FIELD_PLAYER_IN_AREA: AreaID
    303: 0,  # CON_WORLD_FIELD_CHANGE_TROOP
    # Castle
    402: 1, 403: 2, 404: 1, 405: 0, 406: 2, 407: 0, 408: 1,
    409: 3, 410: 1, 411: 1, 412: 0, 413: 0, 414: 0, 415: 2,
    416: 2, 417: 1, 418: 0, 419: 0, 420: 0, 421: 1,
}

COND_NAMES = {
    0: "CON_TIME_ELAPSED", 1: "CON_TIME_ELAPSED_FROM_MARKED",
    2: "CON_TROOP_IN_AREA", 3: "CON_TROOP_SCOUTER_STOPPED_IN_AREA",
    4: "CON_TROOP_SCOUTER_IN_AREA", 5: "CON_TROOP_SCOUTER_CLOSE_TO_TROOP",
    6: "CON_TROOP_CLOSE_TO_TROOP", 7: "CON_TROOP_TARGETED",
    8: "CON_TROOP_ATTACKED", 9: "CON_TROOP_MELEE_ATTACKED",
    10: "CON_TROOP_ABILITY", 11: "CON_TROOP_WITH_THE_SUN_IN_BACK",
    12: "CON_LEADER_HAS_BEEN_KILLED", 13: "CON_STATE_HP_PERCENT",
    14: "CON_LEADER_HP", 15: "CON_OBJECT_LOCATED",
    17: "CON_OBJECT_HP_PERCENT", 18: "CON_OBJECT_DETECTED",
    19: "CON_VAR", 20: "CON_PLAYER_TROOP_ALL_DISABLED",
    22: "CON_PLAYER_ALL_IN_AREA", 23: "CON_TROOP_NOT_IN_AREA",
    24: "CON_TROOP_ATTACKED_TROOP", 25: "CON_PLAYER_TROOP_NOT_IN_AREA",
    26: "CON_PLAYER_TROOP_IN_AREA", 27: "CON_ALWAYS_TRUE",
    28: "CON_PLAYER_KO", 29: "CON_SCOUTER_BACK",
    30: "CON_TROOP_IN_SIGHT", 31: "CON_TROOP_TYPE",
    32: "CON_TROOP_TYPE_IN_AREA", 33: "CON_TROOP_NOT_ENGAGED",
    34: "CON_PLAYER_HP_SUM", 35: "CON_GOT_FIRE",
    36: "CON_CAM_DIR", 37: "CON_IS_DEMO_SKIPPED",
    38: "CON_IS_MINE_AT", 39: "CON_IS_TRAP_AT",
    40: "CON_TROOP_LEADER_TARGET_PLAYER", 41: "CON_PLAYER_ATTACKED",
    42: "CON_DAM_OPENED", 43: "CON_PLAYER_IN_SIGHT",
    44: "CON_RANGE_TROOP_IN_AREA", 45: "CON_PLAYER_CLOSE_TO_TROOP",
    46: "CON_PLAYER_CLOSE_TO_PLAYER", 47: "CON_PLAYER_MELEE_ATTACKED",
    48: "CON_PLAYER_WITH_THE_SUN_IN_BACK", 49: "CON_PLAYER_ATTACKED_TROOP",
    50: "CON_PLAYER_NOT_ENGAGED", 51: "CON_TROOP_UNBLOCKABLE_ATTACKED",
    52: "CON_TROOP_ABILITY_ATTACKED", 53: "CON_IS_WATER_FLOODED_IN_AREA",
    54: "CON_SP", 55: "CON_TROOP_SCALE",
    56: "CON_TROOP_ATTACKED_PLAYER", 57: "CON_TROOP_ATTACKED_BY_FLOOD",
    58: "CON_TROOP_ATTACK_WITH_FACING_THE_SUN",
    59: "CON_PLAYER_SCOUTER_IS_NOT_IN_AREA", 60: "CON_SELECTED_TROOP",
}

# Action param counts from MISSION_SCRIPTING.md
# 'S' = string parameter (null-terminated, padded)
# int = uint32 parameter
# Entries marked with 'S' have special handling
ACT_PARAMS = {
    0: 1,    # ACT_TRIGGER_ACTIVATE: TriggerID
    1: 1,    # ACT_TRIGGER_DEACTIVATE: TriggerID
    2: 1,    # ACT_MARK_ON_TIME: TimeMarkID
    3: 1,    # ACT_POINT_SHOW_IN_MINIMAP: AreaID
    4: 1,    # ACT_POINT_HIDE_IN_MINIMAP: AreaID
    5: 1,    # ACT_TROOP_INDICATE_IN_MINIMAP: TroopID
    6: 2,    # ACT_CHAR_SAY: CharID, TextID
    7: 3,    # ACT_TROOP_SET_PARAM: TroopID, param, int
    8: 1,    # ACT_TROOP_ENABLE: TroopID
    9: 1,    # ACT_TROOP_DISABLE: TroopID
    10: 2,   # ACT_TROOP_WALK_TO: TroopID, AreaID
    11: 2,   # ACT_TROOP_RUN_TO: TroopID, AreaID
    12: 3,   # ACT_TROOP_ADD_WAYPOINT: TroopID, AreaID, int
    13: 3,   # ACT_TROOP_FOLLOW: TroopID, TroopID, float
    14: 1,   # ACT_TROOP_STOP: TroopID
    15: 2,   # ACT_CAM_SET: CameraID, int
    16: 2,   # ACT_CAM_FORCE: CameraID, int
    17: 2,   # ACT_TROOP_RETREAT_TO: TroopID, AreaID
    18: 2,   # ACT_TROOP_ATTACK: TroopID, TroopID
    19: 2,   # ACT_TROOP_SET_TRAP: TroopID, TrapTypeID
    20: 1,   # ACT_TROOP_MORALE_UP: TroopID
    21: 1,   # ACT_SET_CURSOR_POS: AreaID
    22: 0,   # ACT_RESET_ALL_TRIGGERS
    23: 2,   # ACT_ADD_SP: PlayerID, int
    24: 0,   # ACT_RIVER_FLOODED
    26: 3,   # ACT_TROOP_ABILITY: TroopID, AbilityID, AreaID
    27: 3,   # ACT_TROOP_ABILITY_TO_TROOP: TroopID, AbilityID, TroopID
    28: 2,   # ACT_TROOP_ATTACK_LEADER: TroopID, TroopID
    29: 0,   # ACT_HIDE_VAR
    32: 2,   # ACT_VAR_INCREASE: VariableID, int
    # 33: special  # ACT_VAR_DISPLAY: VariableID, int, int, string
    34: 1,   # ACT_SHOW_SKIPPING_MESSAGE: int
    35: 1,   # ACT_TROOP_ANNIHILATED: TroopID
    38: 1,   # ACT_OPEN_SESAME: PropID
    39: 1,   # ACT_CLOSE_SESAME: PropID
    47: 0,   # ACT_EVENT_ANCIENT_HEART_CALLED_ME
    49: 0,   # ACT_MISSION_COMPLETE
    50: 0,   # ACT_MISSION_FAIL
    51: 1,   # ACT_DELAY_TICK: int
    52: 0,   # ACT_LOOP
    53: 0,   # ACT_RESET_TRIGGER
    54: 1,   # ACT_SHOW_TEXT: TextID
    55: 2,   # ACT_VAR_INT_SET: VariableID, int
    56: 2,   # ACT_VAR_RANDOM_SET: VariableID, float
    57: 0,   # ACT_CAM_RESET
    58: 0,   # ACT_LETTER_BOX_ENABLE
    59: 0,   # ACT_LETTER_BOX_DISABLE
    60: 2,   # ACT_SHOW_TEXT_EX: TextID, int
    61: 1,   # ACT_RESET_TRIGGER_EX: TriggerID
    62: 2,   # ACT_TROOP_SIGNAL: TroopID, animID
    63: 1,   # ACT_BLOCK_AREA: AreaID
    64: 1,   # ACT_OPEN_AREA: AreaID
    65: 1,   # ACT_RECOVER_AREA: AreaID
    66: 2,   # ACT_SET_AI: TroopID, AI_ID
    67: 1,   # ACT_ENABLE_AI: TroopID
    68: 1,   # ACT_DISABLE_AI: TroopID
    70: 4,   # ACT_SHOW_TEXT_XY_2: x, y, int, TextID
    71: 1,   # ACT_SET_SNOW: int
    72: 0,   # ACT_REMOVE_SNOW
    73: 1,   # ACT_SET_CAM_TARGET: TroopID
    74: 0,   # ACT_UNSET_CAM_TARGET
    75: 1,   # ACT_RENEW_TROOP: TroopID
    76: 3,   # ACT_SHOW_TITLE: x, y, int
    77: 4,   # ACT_SHOW_TEXT_XY: x, y, int, TextID
    78: 1,   # ACT_SET_FPS: int
    79: 0,   # ACT_RESET_FPS
    80: 1,   # ACT_SET_MOTION_BLUR: int
    81: 0,   # ACT_RESET_MOTION_BLUR
    82: 2,   # ACT_TROOP_SET_SPEED: TroopID, float
    83: 1,   # ACT_TROOP_RESET_SPEED: TroopID
    84: 1,   # ACT_SET_RAIN: int
    85: 0,   # ACT_STOP_RAIN
    86: 2,   # ACT_SET_WIND: int, float
    87: 3,   # ACT_SET_GATE: AreaID, PlayerGroupID, int
    88: 0,   # ACT_START_WATER_ATTACK
    89: 3,   # ACT_CHAR_SAY_EX: CharID, TextID, int
    90: 1,   # ACT_LEADER_INVULNERABLE: TroopID
    91: 1,   # ACT_LEADER_VULNERABLE: TroopID
    92: 1,   # ACT_LEADER_RECHARGE_RATE: TroopID
    93: 2,   # ACT_TROOP_SET_BOUNDARY: TroopID, AreaID
    94: 1,   # ACT_TROOP_RESET_BOUNDARY: TroopID
    95: 3,   # ACT_TROOP_WARP: TroopID, AreaID, int
    96: 1,   # ACT_MY_PLAYER_GET_EXP: float
    97: 1,   # ACT_SHOW_AREA_ON_MINIMAP: AreaID
    98: 1,   # ACT_HIDE_AREA_ON_MINIMAP: AreaID
    99: 3,   # ACT_SHOW_TEXT_ON_MSG_WINDOW: TextID, int, int
    100: 2,  # ACT_TROOP_SIGNAL_ARROW: TroopID, AreaID
    101: 1,  # ACT_FADE_IN: int
    102: 1,  # ACT_FADE_OUT: int
    103: 0,  # ACT_OPEN_DAM
    104: 0,  # ACT_CLOSE_DAM
    105: 2,  # ACT_DISABLE_TROOPS_INSIDE_AREA: AreaID, PlayerID
    106: 2,  # ACT_DISABLE_TROOPS_OUTSIDE_AREA: AreaID, PlayerID
    107: 0,  # ACT_DISABLE_ALL_TROOPS
    109: 1,  # ACT_COLLAPSE_WALL: index
    # 110: special  # ACT_PLAY_BGM: string, int, int
    111: 1,  # ACT_STOP_BGM: int
    112: 1,  # ACT_START_TROOP_INDICATE_IN_MINIMAP: TroopID
    113: 1,  # ACT_STOP_TROOP_INDICATE_IN_MINIMAP: TroopID
    114: 1,  # ACT_TROOP_REFILL_HP: TroopID
    115: 2,  # ACT_TROOP_SET_HP: TroopID, int%
    116: 2,  # ACT_LIP_SYNC_BEGIN: TroopID, int
    117: 2,  # ACT_LIP_SYNC_END: TroopID, int
    118: 1,  # ACT_SET_FIRE_SPREAD_SPEED: float
    119: 0,  # ACT_ENABLE_INPUT
    120: 0,  # ACT_DISABLE_INPUT
    121: 1,  # ACT_ENABLE_FOG_OF_WAR: int
    122: 1,  # ACT_SET_FIRE_SPREAD_RANGE: float
    123: 1,  # ACT_SET_FIRE: AreaID
    124: 1,  # ACT_REGNIER_GO_CRAZY: TroopID
    125: 1,  # ACT_REGNIER_FREE_HIS_POWER: TroopID
    126: 1,  # ACT_SET_MUTE: int
    127: 1,  # ACT_BURY_TROOP: TroopID
    # 128: special  # ACT_OFFICER_SAY: complex
    129: 2,  # ACT_TAG_THE_TROOP: TroopID, int
    130: 1,  # ACT_UNTAG_THE_TROOP: TroopID
    131: 3,  # ACT_SHOW_VAR_GAUGE: PortraitID, VarID, TextID
    132: 0,  # ACT_HIDE_VAR_GAUGE
    133: 2,  # ACT_TROOP_ANIMATION: TroopID, animID
    135: 2,  # ACT_TROOP_RANGE_ATTACK_ON_POS: TroopID, AreaID
    136: 2,  # ACT_TROOP_RANGE_ATTACK_ON_PROP: TroopID, PropID
    # 137: special  # ACT_SET_AI_PATH: PathID, AreaIDs... (variable)
    138: 1,  # ACT_ENABLE_ABILITY: AbilityID
    139: 1,  # ACT_DISABLE_ABILITY: AbilityID
    140: 1,  # ACT_SHOW_NOISE_METER_GAUGE: TextID
    141: 2,  # ACT_SET_BGM_VOLUME: int, int%
    142: 3,  # ACT_FADE_BGM: int, int, int%
    # 143: special  # ACT_MARK_ON_TROOP_IN_AREA: AreaID, PlayerID, int
    143: 3,
    144: 2,  # ACT_SET_WALL_HP: PropID, int
    145: 1,  # ACT_TROOP_SET_INVULNERABLE: TroopID
    146: 1,  # ACT_TROOP_RESET_INVULNERABLE: TroopID
    147: 1,  # ACT_TROOP_SELECT: TroopID
    148: 0,  # ACT_FORCE_MINIMAP_ON
    149: 0,  # ACT_FORCE_MINIMAP_OFF
    # 150: special  # ACT_SHOW_PROP_HP_GAUGE: variable params
    # 151: special  # ACT_SHOW_TROOP_HP_GAUGE: variable params
    # 152: special  # ACT_RENEW_TROOP_OUTOFSIGHT: TroopID, AreaID, PlayerID, int
    152: 4,
    153: 3,  # ACT_SET_TRAP: AreaID, PlayerID, int
    154: 3,  # ACT_SET_MINE: AreaID, PlayerID, int
    155: 0,  # ACT_FLOOD_RESET
    157: 0,  # ACT_ENABLE_PAUSE
    158: 0,  # ACT_DISABLE_PAUSE
    159: 0,  # ACT_QUICK_SAVE
    # 160: special  # ACT_CHAR_RANDOM_SAY: variable params
    161: 2,  # ACT_ENABLE_TROOP_IN_AREA: AreaID, PlayerID
    162: 1,  # ACT_PLAYER_TROOP_STOP: PlayerID
    163: 0,  # ACT_EXCLUSIVE_BEGIN
    164: 0,  # ACT_EXCLUSIVE_END
    # 165: special  # ACT_LOAD_MISSION: string, int
    166: 1,  # ACT_JOYPAD_RUMBLE: int
    # 167: special  # ACT_SHOW_TROOP_DIST_GAUGE: variable
    168: 2,  # ACT_SET_FIRE_N_SMOKE: AreaID, int
    169: 1,  # ACT_SET_WATER_EFFECT_PROP: AreaID
    170: 1,  # ACT_SET_SCREEN_GLOW: int
    171: 2,  # ACT_UPDATE_UNIT_KILL_COUNT: VarID, PlayerID
    # 172: special  # ACT_PLAY_FMV: string, string
    173: 1,  # ACT_ENABLE_LENS_FLARE: int
    # 174: special  # ACT_CHANGE_SKYBOX_N_LIGHT_SET: string, string
    175: 2,  # ACT_REMOVE_TRAP: AreaID, PlayerID
    176: 2,  # ACT_SET_FIRE_N_SMOKE_SMALL: AreaID, int
    177: 2,  # ACT_SET_TRAINING_MISSION: int, int
    178: 2,  # ACT_SET_LIBRARY: int, int
    179: 0,  # ACT_ALL_MISSION_COMPLETE
    # 180: special  # ACT_PLAY_FMV_AND_GO_TO_WORLDMAP: string
    181: 0,  # ACT_GO_TO_WORLDMAP
    182: 0,  # ACT_SKIP_TEXT
    # Briefing
    500: 2, 501: 3, 502: 3, 503: 3, 504: 3, 505: 0, 506: 0,
    507: 1, 508: 1, 509: 1, 510: 0, 511: 4, 512: 1, 513: 3, 514: 4,
    # Worldmap
    700: 2, 701: 1, 702: 1, 703: 2, 704: 2, 706: 0, 707: 0, 708: 0,
    # 709: special  # string, string
    710: 0, 711: 2, 712: 2, 713: 0, 714: 0, 715: 1, 716: 1,
    717: 1, 718: 0, 719: 0, 720: 1, 721: 2, 723: 1, 724: 0,
    725: 2, 726: 3, 727: 1, 728: 3,
    # 729: special  # string, int, int
    730: 1, 731: 2, 732: 3, 733: 2, 734: 2, 735: 2, 736: 1,
    737: 3, 738: 3, 739: 2,
    # Live
    300: 1,
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
    47: "ACT_EVENT_ANCIENT_HEART_CALLED_ME",
    49: "ACT_MISSION_COMPLETE", 50: "ACT_MISSION_FAIL",
    51: "ACT_DELAY_TICK", 52: "ACT_LOOP", 53: "ACT_RESET_TRIGGER",
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
    92: "ACT_LEADER_RECHARGE_RATE", 93: "ACT_TROOP_SET_BOUNDARY",
    94: "ACT_TROOP_RESET_BOUNDARY", 95: "ACT_TROOP_WARP",
    96: "ACT_MY_PLAYER_GET_EXP", 97: "ACT_SHOW_AREA_ON_MINIMAP",
    98: "ACT_HIDE_AREA_ON_MINIMAP", 99: "ACT_SHOW_TEXT_ON_MSG_WINDOW",
    100: "ACT_TROOP_SIGNAL_ARROW", 101: "ACT_FADE_IN",
    102: "ACT_FADE_OUT", 103: "ACT_OPEN_DAM", 104: "ACT_CLOSE_DAM",
    105: "ACT_DISABLE_TROOPS_INSIDE_AREA", 106: "ACT_DISABLE_TROOPS_OUTSIDE_AREA",
    107: "ACT_DISABLE_ALL_TROOPS", 109: "ACT_COLLAPSE_WALL",
    110: "ACT_PLAY_BGM", 111: "ACT_STOP_BGM",
    112: "ACT_START_TROOP_INDICATE", 113: "ACT_STOP_TROOP_INDICATE",
    114: "ACT_TROOP_REFILL_HP", 115: "ACT_TROOP_SET_HP",
    116: "ACT_LIP_SYNC_BEGIN", 117: "ACT_LIP_SYNC_END",
    118: "ACT_SET_FIRE_SPREAD_SPEED", 119: "ACT_ENABLE_INPUT",
    120: "ACT_DISABLE_INPUT", 121: "ACT_ENABLE_FOG_OF_WAR",
    122: "ACT_SET_FIRE_SPREAD_RANGE", 123: "ACT_SET_FIRE",
    124: "ACT_REGNIER_GO_CRAZY", 125: "ACT_REGNIER_FREE_HIS_POWER",
    126: "ACT_SET_MUTE", 127: "ACT_BURY_TROOP",
    128: "ACT_OFFICER_SAY", 129: "ACT_TAG_THE_TROOP",
    130: "ACT_UNTAG_THE_TROOP", 131: "ACT_SHOW_VAR_GAUGE",
    132: "ACT_HIDE_VAR_GAUGE", 133: "ACT_TROOP_ANIMATION",
    135: "ACT_TROOP_RANGE_ATTACK_POS", 136: "ACT_TROOP_RANGE_ATTACK_PROP",
    137: "ACT_SET_AI_PATH", 138: "ACT_ENABLE_ABILITY",
    139: "ACT_DISABLE_ABILITY", 140: "ACT_SHOW_NOISE_METER",
    141: "ACT_SET_BGM_VOLUME", 142: "ACT_FADE_BGM",
    143: "ACT_MARK_ON_TROOP_IN_AREA", 144: "ACT_SET_WALL_HP",
    145: "ACT_TROOP_SET_INVULNERABLE", 146: "ACT_TROOP_RESET_INVULNERABLE",
    147: "ACT_TROOP_SELECT", 148: "ACT_FORCE_MINIMAP_ON",
    149: "ACT_FORCE_MINIMAP_OFF", 150: "ACT_SHOW_PROP_HP_GAUGE",
    151: "ACT_SHOW_TROOP_HP_GAUGE", 152: "ACT_RENEW_TROOP_OUTOFSIGHT",
    153: "ACT_SET_TRAP", 154: "ACT_SET_MINE", 155: "ACT_FLOOD_RESET",
    157: "ACT_ENABLE_PAUSE", 158: "ACT_DISABLE_PAUSE",
    159: "ACT_QUICK_SAVE", 160: "ACT_CHAR_RANDOM_SAY",
    161: "ACT_ENABLE_TROOP_IN_AREA", 162: "ACT_PLAYER_TROOP_STOP",
    163: "ACT_EXCLUSIVE_BEGIN", 164: "ACT_EXCLUSIVE_END",
    165: "ACT_LOAD_MISSION", 166: "ACT_JOYPAD_RUMBLE",
    167: "ACT_SHOW_TROOP_DIST_GAUGE", 168: "ACT_SET_FIRE_N_SMOKE",
    169: "ACT_SET_WATER_EFFECT_PROP", 170: "ACT_SET_SCREEN_GLOW",
    171: "ACT_UPDATE_UNIT_KILL_COUNT", 172: "ACT_PLAY_FMV",
    173: "ACT_ENABLE_LENS_FLARE", 174: "ACT_CHANGE_SKYBOX_N_LIGHT",
    175: "ACT_REMOVE_TRAP", 176: "ACT_SET_FIRE_N_SMOKE_SMALL",
    177: "ACT_SET_TRAINING_MISSION", 178: "ACT_SET_LIBRARY",
    179: "ACT_ALL_MISSION_COMPLETE", 180: "ACT_PLAY_FMV_AND_GO_WORLDMAP",
    181: "ACT_GO_TO_WORLDMAP", 182: "ACT_SKIP_TEXT",
}

# Actions with string parameters - return (int_params_before_string, string_count, int_params_after_string)
# We need to discover how strings are encoded!
STRING_ACTIONS = {110, 165, 172, 174, 180, 709, 729}
STRING_ACTIONS_2 = {33}  # Also has string

def parse_stg_tail_start(data):
    """Parse the tail section to find where events start."""
    header_size = 628
    unit_size = 544
    unit_count = read_u32(data, 0x270)

    tail_offset = header_size + unit_count * unit_size
    print(f"  Units: {unit_count}, tail starts at 0x{tail_offset:X}")

    # Areas
    area_count = read_u32(data, tail_offset)
    area_section_size = 4 + area_count * 84
    print(f"  Areas: {area_count} ({area_section_size} bytes)")

    # Variables (76 bytes each: 64-byte name + 12 bytes data)
    var_offset = tail_offset + area_section_size
    var_count = read_u32(data, var_offset)
    var_section_size = 4 + var_count * 76
    print(f"  Variables: {var_count} ({var_section_size} bytes)")
    for i in range(var_count):
        vbase = var_offset + 4 + i * 76
        vname = read_cstr(data, vbase, 64)
        vid = read_u32(data, vbase + 64)
        vpad = read_u32(data, vbase + 68)
        vval = read_u32(data, vbase + 72)
        print(f"    var[{i}]: '{vname}' id={vid} pad={vpad} initial={vval}")

    # What comes after variables?
    after_vars = var_offset + var_section_size
    print(f"\n  After variables at 0x{after_vars:X}:")
    hexdump(data, after_vars, 32)

    # Try different interpretations
    v1 = read_u32(data, after_vars)
    v2 = read_u32(data, after_vars + 4)
    v3 = read_u32(data, after_vars + 8)
    print(f"\n  Interpretation A: mystery({v1}, {v2}) + event_count={v3}")
    print(f"    events at 0x{after_vars + 12:X}")

    print(f"  Interpretation B: event_count={v1}")
    print(f"    events at 0x{after_vars + 4:X}")

    # Use the "mystery fields" interpretation (matches previous session)
    event_count = v3
    events_offset = after_vars + 12

    # But also try direct event_count
    alt_event_count = v1
    alt_events_offset = after_vars + 4

    return events_offset, event_count, alt_events_offset, alt_event_count


def try_parse_variable_entries(data, events_offset, event_count, file_size, label=""):
    """Parse events with variable-length condition/action entries using lookup tables."""
    print(f"\n=== Variable-length entries (lookup-based) {label} ===")
    print(f"  events at 0x{events_offset:X}, count={event_count}")

    offset = events_offset
    success_count = 0

    for i in range(min(event_count, 50)):
        if offset + 72 > file_size:
            print(f"  Event {i}: OUT OF BOUNDS at 0x{offset:X}")
            break

        desc = read_cstr(data, offset, 64)
        blockId = read_u32(data, offset + 64)
        numCond = read_u32(data, offset + 68)

        if numCond > 50:
            print(f"  Event {i} at 0x{offset:X}: numCond={numCond} INVALID")
            print(f"    Context:")
            hexdump(data, offset, min(80, file_size - offset))
            break

        pos = offset + 72
        conds = []
        valid = True

        for c in range(numCond):
            if pos + 4 > file_size:
                print(f"  Event {i}: cond[{c}] past EOF at 0x{pos:X}")
                valid = False
                break
            cid = read_u32(data, pos)
            pc = COND_PARAMS.get(cid)
            if pc is None:
                print(f"  Event {i}: cond[{c}] UNKNOWN id={cid} at 0x{pos:X}")
                hexdump(data, pos, min(32, file_size - pos))
                valid = False
                break
            if pos + 4 + pc * 4 > file_size:
                print(f"  Event {i}: cond[{c}] params past EOF")
                valid = False
                break
            params = [read_u32(data, pos + 4 + p*4) for p in range(pc)]
            name = COND_NAMES.get(cid, f"CON_{cid}")
            conds.append(f"{name}({','.join(str(p) for p in params)})")
            pos += 4 + pc * 4

        if not valid:
            break

        if pos + 4 > file_size:
            print(f"  Event {i}: numAct field past EOF at 0x{pos:X}")
            break

        numAct = read_u32(data, pos)
        pos += 4

        if numAct > 100:
            print(f"  Event {i} at 0x{offset:X}: numAct={numAct} INVALID at 0x{pos-4:X}")
            hexdump(data, pos - 8, min(48, file_size - pos + 8))
            break

        acts = []
        for a in range(numAct):
            if pos + 4 > file_size:
                print(f"  Event {i}: act[{a}] past EOF at 0x{pos:X}")
                valid = False
                break
            aid = read_u32(data, pos)

            if aid in STRING_ACTIONS:
                # Handle string-containing actions
                # Try: id(4) + int_params... + string_len(4) + string[padded]
                name = ACT_NAMES.get(aid, f"ACT_{aid}")
                acts.append(f"{name}(STRING_ACTION at 0x{pos:X})")
                print(f"  Event {i}: act[{a}] = {name} (STRING action) at 0x{pos:X}")
                print(f"    Context:")
                hexdump(data, pos, min(80, file_size - pos))
                valid = False
                break

            pc = ACT_PARAMS.get(aid)
            if pc is None:
                print(f"  Event {i}: act[{a}] UNKNOWN id={aid} at 0x{pos:X}")
                hexdump(data, pos, min(32, file_size - pos))
                valid = False
                break
            if pos + 4 + pc * 4 > file_size:
                print(f"  Event {i}: act[{a}] params past EOF")
                valid = False
                break
            params = [read_u32(data, pos + 4 + p*4) for p in range(pc)]
            name = ACT_NAMES.get(aid, f"ACT_{aid}")
            acts.append(f"{name}({','.join(str(p) for p in params)})")
            pos += 4 + pc * 4

        if not valid:
            break

        event_size = pos - offset
        print(f"  Event {i} at 0x{offset:X}: size={event_size} desc='{desc[:40]}' blockId={blockId}")
        print(f"    numCond={numCond}: {' '.join(conds)}")
        print(f"    numAct={numAct}: {' '.join(acts[:5])}")
        if len(acts) > 5:
            print(f"      ... and {len(acts)-5} more")
        print(f"    next at 0x{pos:X}")

        success_count += 1
        offset = pos

    print(f"\n  Successfully parsed {success_count}/{event_count} events")
    return success_count


def try_fixed16_entries(data, events_offset, event_count, file_size, label=""):
    """Theory: all entries are fixed 16 bytes (id + 3 params)."""
    print(f"\n=== Fixed 16-byte entries {label} ===")
    print(f"  events at 0x{events_offset:X}, count={event_count}")

    offset = events_offset
    success_count = 0

    for i in range(min(event_count, 50)):
        if offset + 72 > file_size:
            print(f"  Event {i}: OUT OF BOUNDS at 0x{offset:X}")
            break

        desc = read_cstr(data, offset, 64)
        blockId = read_u32(data, offset + 64)
        numCond = read_u32(data, offset + 68)

        if numCond > 50:
            print(f"  Event {i} at 0x{offset:X}: numCond={numCond} INVALID")
            break

        cond_end = offset + 72 + numCond * 16
        if cond_end + 4 > file_size:
            print(f"  Event {i}: conditions extend past EOF")
            break

        numAct = read_u32(data, cond_end)
        if numAct > 100:
            print(f"  Event {i} at 0x{offset:X}: numAct={numAct} INVALID")
            break

        act_end = cond_end + 4 + numAct * 16
        if act_end > file_size:
            print(f"  Event {i}: actions extend past EOF")
            break

        event_size = act_end - offset

        # Print conditions
        conds = []
        for c in range(numCond):
            base = offset + 72 + c * 16
            cid = read_u32(data, base)
            p1, p2, p3 = read_u32(data, base+4), read_u32(data, base+8), read_u32(data, base+12)
            name = COND_NAMES.get(cid, f"CON_{cid}")
            conds.append(f"{name}({p1},{p2},{p3})")

        # Print actions
        acts = []
        for a in range(numAct):
            base = cond_end + 4 + a * 16
            aid = read_u32(data, base)
            p1, p2, p3 = read_u32(data, base+4), read_u32(data, base+8), read_u32(data, base+12)
            name = ACT_NAMES.get(aid, f"ACT_{aid}")
            acts.append(f"{name}({p1},{p2},{p3})")

        print(f"  Event {i} at 0x{offset:X}: size={event_size} desc='{desc[:40]}' blockId={blockId}")
        print(f"    numCond={numCond}: {' '.join(conds)}")
        print(f"    numAct={numAct}: {' '.join(acts[:5])}")
        if len(acts) > 5:
            print(f"      ... and {len(acts)-5} more")

        # Check if next event looks valid
        next_off = act_end
        if next_off + 72 <= file_size and i + 1 < event_count:
            next_desc = read_cstr(data, next_off, 64)
            next_blockId = read_u32(data, next_off + 64)
            # Is the next block plausible?
            has_printable = any(32 <= data[next_off + j] < 127 or data[next_off + j] == 0 for j in range(min(64, file_size - next_off)))
            print(f"    next at 0x{next_off:X}: desc='{next_desc[:30]}' blockId={next_blockId} {'OK' if next_blockId < 1000 else 'SUSPICIOUS'}")

        success_count += 1
        offset = act_end

    print(f"\n  Successfully parsed {success_count}/{event_count} events")
    return success_count


def analyze_file(filepath):
    data = Path(filepath).read_bytes()
    print(f"\n{'='*70}")
    print(f"File: {filepath} ({len(data)} bytes)")
    print(f"{'='*70}")

    events_offset, event_count, alt_offset, alt_count = parse_stg_tail_start(data)

    # Show first 128 bytes of event area
    print(f"\n  First events data at 0x{events_offset:X}:")
    hexdump(data, events_offset, min(128, len(data) - events_offset))

    # Also show data using alt interpretation
    if alt_offset != events_offset:
        print(f"\n  Alt events data at 0x{alt_offset:X}:")
        hexdump(data, alt_offset, min(128, len(data) - alt_offset))

    # Remaining bytes
    remaining = len(data) - events_offset
    print(f"\n  Remaining from events: {remaining} bytes")
    print(f"  If 448-byte events: need {event_count * 448 + 42} bytes (have {remaining})")
    if event_count > 0:
        print(f"  Average event size if no footer: {remaining / event_count:.1f}")
        print(f"  Average event size with 42-byte footer: {(remaining - 42) / event_count:.1f}")

    # Try all parsing approaches with both offsets
    print(f"\n--- Using mystery(1,0) + event_count={event_count} ---")
    try_fixed16_entries(data, events_offset, event_count, len(data), "mystery+count")
    try_parse_variable_entries(data, events_offset, event_count, len(data), "mystery+count")

    if alt_count != event_count and alt_count < 100:
        print(f"\n--- Using direct event_count={alt_count} (no mystery) ---")
        try_fixed16_entries(data, alt_offset, alt_count, len(data), "direct-count")
        try_parse_variable_entries(data, alt_offset, alt_count, len(data), "direct-count")

    # Show last 50 bytes (potential footer)
    print(f"\n  Last 50 bytes of file:")
    hexdump(data, max(0, len(data) - 50), 50)


if __name__ == '__main__':
    mission_dir = Path.home() / "Downloads" / "KUF Crusaders" / "Mission"

    for name in ["E1140.stg", "X3001.stg", "E1001.stg", "TUTO_D1.stg"]:
        path = mission_dir / name
        if path.exists():
            analyze_file(path)
        else:
            print(f"NOT FOUND: {path}")
