#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct STGScriptInfo {
    pub id: u32,
    pub name: &'static str,
    pub parameter_count: u32,
    pub parameter_hints: [&'static str; 3],
}

pub fn conditions() -> &'static [STGScriptInfo] {
    &CONDITIONS
}

pub fn actions() -> &'static [STGScriptInfo] {
    &ACTIONS
}

pub fn condition(id: u32) -> Option<&'static STGScriptInfo> {
    lookup(&CONDITIONS, id)
}

pub fn action(id: u32) -> Option<&'static STGScriptInfo> {
    lookup(&ACTIONS, id)
}

fn lookup(entries: &'static [STGScriptInfo], id: u32) -> Option<&'static STGScriptInfo> {
    entries
        .binary_search_by_key(&id, |entry| entry.id)
        .ok()
        .and_then(|index| entries.get(index))
}

macro_rules! script {
    ($id:literal, $name:literal, $count:literal, [$first:literal, $second:literal, $third:literal]) => {
        STGScriptInfo {
            id: $id,
            name: $name,
            parameter_count: $count,
            parameter_hints: [$first, $second, $third],
        }
    };
}

static CONDITIONS: [STGScriptInfo; 59] = [
    script!(0, "CON_TIME_ELAPSED", 2, ["Seconds", "Compare", ""]),
    script!(
        1,
        "CON_TIME_ELAPSED_FROM_MARKED",
        3,
        ["TimeMarkID", "Seconds", "Compare"]
    ),
    script!(2, "CON_TROOP_IN_AREA", 2, ["TroopID", "AreaID", ""]),
    script!(
        3,
        "CON_TROOP_SCOUTER_STOPPED_IN_AREA",
        2,
        ["TroopID", "AreaID", ""]
    ),
    script!(4, "CON_TROOP_SCOUTER_IN_AREA", 2, ["TroopID", "AreaID", ""]),
    script!(
        5,
        "CON_TROOP_SCOUTER_CLOSE_TO_TROOP",
        3,
        ["TroopID", "TroopID2", "Distance"]
    ),
    script!(
        6,
        "CON_TROOP_CLOSE_TO_TROOP",
        3,
        ["TroopID", "TroopID2", "Distance"]
    ),
    script!(7, "CON_TROOP_TARGETED", 2, ["TroopID", "PlayerID", ""]),
    script!(8, "CON_TROOP_ATTACKED", 1, ["TroopID", "", ""]),
    script!(9, "CON_TROOP_MELEE_ATTACKED", 1, ["TroopID", "", ""]),
    script!(10, "CON_TROOP_ABILITY", 2, ["TroopID", "AbilityID", ""]),
    script!(
        11,
        "CON_TROOP_WITH_THE_SUN_IN_BACK",
        2,
        ["TroopID", "Value", ""]
    ),
    script!(12, "CON_LEADER_HAS_BEEN_KILLED", 1, ["TroopID", "", ""]),
    script!(
        13,
        "CON_STATE_HP_PERCENT",
        3,
        ["TroopID", "Percent", "Compare"]
    ),
    script!(14, "CON_LEADER_HP", 3, ["TroopID", "Percent", "Compare"]),
    script!(15, "CON_OBJECT_LOCATED", 2, ["AreaID", "ObjectType", ""]),
    script!(
        17,
        "CON_OBJECT_HP_PERCENT",
        3,
        ["ObjectID", "Percent", "Compare"]
    ),
    script!(18, "CON_OBJECT_DETECTED", 1, ["ObjectType", "", ""]),
    script!(19, "CON_VAR", 3, ["VariableID", "Value", "Compare"]),
    script!(
        20,
        "CON_PLAYER_TROOP_ALL_DISABLED",
        2,
        ["PlayerID", "Percent", ""]
    ),
    script!(22, "CON_PLAYER_ALL_IN_AREA", 2, ["PlayerID", "AreaID", ""]),
    script!(23, "CON_TROOP_NOT_IN_AREA", 2, ["TroopID", "AreaID", ""]),
    script!(
        24,
        "CON_TROOP_ATTACKED_TROOP",
        2,
        ["TroopID", "TargetID", ""]
    ),
    script!(
        25,
        "CON_PLAYER_TROOP_NOT_IN_AREA",
        2,
        ["AreaID", "PlayerID", ""]
    ),
    script!(
        26,
        "CON_PLAYER_TROOP_IN_AREA",
        2,
        ["AreaID", "PlayerID", ""]
    ),
    script!(27, "CON_ALWAYS_TRUE", 0, ["", "", ""]),
    script!(28, "CON_PLAYER_KO", 1, ["PlayerID", "", ""]),
    script!(29, "CON_SCOUTER_BACK", 1, ["TroopID", "", ""]),
    script!(30, "CON_TROOP_IN_SIGHT", 1, ["TroopID", "", ""]),
    script!(31, "CON_TROOP_TYPE", 2, ["TroopID", "TroopType", ""]),
    script!(
        32,
        "CON_TROOP_TYPE_IN_AREA",
        3,
        ["PlayerID", "TroopType", "AreaID"]
    ),
    script!(33, "CON_TROOP_NOT_ENGAGED", 1, ["TroopID", "", ""]),
    script!(
        34,
        "CON_PLAYER_HP_SUM",
        3,
        ["PlayerID", "Percent", "Compare"]
    ),
    script!(35, "CON_GOT_FIRE", 1, ["AreaID", "", ""]),
    script!(36, "CON_CAM_DIR", 1, ["AreaID", "", ""]),
    script!(37, "CON_IS_DEMO_SKIPPED", 0, ["", "", ""]),
    script!(38, "CON_IS_MINE_AT", 2, ["PlayerID", "AreaID", ""]),
    script!(39, "CON_IS_TRAP_AT", 2, ["PlayerID", "AreaID", ""]),
    script!(40, "CON_TROOP_LEADER_TARGET_PLAYER", 1, ["TroopID", "", ""]),
    script!(41, "CON_PLAYER_ATTACKED", 1, ["PlayerID", "", ""]),
    script!(42, "CON_DAM_OPENED", 0, ["", "", ""]),
    script!(43, "CON_PLAYER_IN_SIGHT", 1, ["PlayerID", "", ""]),
    script!(44, "CON_RANGE_TROOP_IN_AREA", 2, ["AreaID", "PlayerID", ""]),
    script!(
        45,
        "CON_PLAYER_CLOSE_TO_TROOP",
        3,
        ["PlayerID", "TroopID", "Distance"]
    ),
    script!(
        46,
        "CON_PLAYER_CLOSE_TO_PLAYER",
        3,
        ["PlayerID", "PlayerID2", "Distance"]
    ),
    script!(47, "CON_PLAYER_MELEE_ATTACKED", 1, ["PlayerID", "", ""]),
    script!(
        48,
        "CON_PLAYER_WITH_THE_SUN_IN_BACK",
        1,
        ["PlayerID", "", ""]
    ),
    script!(
        49,
        "CON_PLAYER_ATTACKED_TROOP",
        2,
        ["PlayerID", "TroopID", ""]
    ),
    script!(50, "CON_PLAYER_NOT_ENGAGED", 1, ["PlayerID", "", ""]),
    script!(51, "CON_TROOP_UNBLOCKABLE_ATTACKED", 1, ["TroopID", "", ""]),
    script!(
        52,
        "CON_TROOP_ABILITY_ATTACKED",
        2,
        ["TroopID", "AbilityID", ""]
    ),
    script!(53, "CON_IS_WATER_FLOODED_IN_AREA", 1, ["AreaID", "", ""]),
    script!(54, "CON_SP", 3, ["PlayerID", "Value", "Compare"]),
    script!(55, "CON_TROOP_SCALE", 2, ["TroopID", "ScaleType", ""]),
    script!(
        56,
        "CON_TROOP_ATTACKED_PLAYER",
        2,
        ["TroopID", "PlayerID", ""]
    ),
    script!(57, "CON_TROOP_ATTACKED_BY_FLOOD", 1, ["TroopID", "", ""]),
    script!(
        58,
        "CON_TROOP_ATTACK_WITH_FACING_THE_SUN",
        1,
        ["TroopID", "", ""]
    ),
    script!(
        59,
        "CON_PLAYER_SCOUTER_IS_NOT_IN_AREA",
        2,
        ["PlayerID", "AreaID", ""]
    ),
    script!(60, "CON_SELECTED_TROOP", 1, ["TroopID", "", ""]),
];

static ACTIONS: [STGScriptInfo; 167] = [
    script!(0, "ACT_TRIGGER_ACTIVATE", 1, ["TriggerID", "", ""]),
    script!(1, "ACT_TRIGGER_DEACTIVATE", 1, ["TriggerID", "", ""]),
    script!(2, "ACT_MARK_ON_TIME", 1, ["TimeMarkID", "", ""]),
    script!(3, "ACT_POINT_SHOW_IN_MINIMAP", 1, ["AreaID", "", ""]),
    script!(4, "ACT_POINT_HIDE_IN_MINIMAP", 1, ["AreaID", "", ""]),
    script!(5, "ACT_TROOP_INDICATE_IN_MINIMAP", 1, ["TroopID", "", ""]),
    script!(6, "ACT_CHAR_SAY", 2, ["CharID", "TextID", ""]),
    script!(7, "ACT_TROOP_SET_PARAM", 3, ["TroopID", "Param", "Value"]),
    script!(8, "ACT_TROOP_ENABLE", 1, ["TroopID", "", ""]),
    script!(9, "ACT_TROOP_DISABLE", 1, ["TroopID", "", ""]),
    script!(10, "ACT_TROOP_WALK_TO", 2, ["TroopID", "AreaID", ""]),
    script!(11, "ACT_TROOP_RUN_TO", 2, ["TroopID", "AreaID", ""]),
    script!(
        12,
        "ACT_TROOP_ADD_WAYPOINT",
        3,
        ["TroopID", "AreaID", "MoveType"]
    ),
    script!(
        13,
        "ACT_TROOP_FOLLOW",
        3,
        ["TroopID", "TargetID", "Distance"]
    ),
    script!(14, "ACT_TROOP_STOP", 1, ["TroopID", "", ""]),
    script!(15, "ACT_CAM_SET", 2, ["CameraID", "Duration", ""]),
    script!(16, "ACT_CAM_FORCE", 2, ["CameraID", "Duration", ""]),
    script!(17, "ACT_TROOP_RETREAT_TO", 2, ["TroopID", "AreaID", ""]),
    script!(18, "ACT_TROOP_ATTACK", 2, ["TroopID", "TargetID", ""]),
    script!(19, "ACT_TROOP_SET_TRAP", 2, ["TroopID", "TrapTypeID", ""]),
    script!(20, "ACT_TROOP_MORALE_UP", 1, ["TroopID", "", ""]),
    script!(21, "ACT_SET_CURSOR_POS", 1, ["AreaID", "", ""]),
    script!(22, "ACT_RESET_ALL_TRIGGERS", 0, ["", "", ""]),
    script!(23, "ACT_ADD_SP", 2, ["PlayerID", "SP", ""]),
    script!(24, "ACT_RIVER_FLOODED", 0, ["", "", ""]),
    script!(
        26,
        "ACT_TROOP_ABILITY",
        3,
        ["TroopID", "AbilityID", "AreaID"]
    ),
    script!(
        27,
        "ACT_TROOP_ABILITY_TO_TROOP",
        3,
        ["TroopID", "AbilityID", "TargetID"]
    ),
    script!(
        28,
        "ACT_TROOP_ATTACK_LEADER",
        2,
        ["TroopID", "TargetID", ""]
    ),
    script!(29, "ACT_HIDE_VAR", 0, ["", "", ""]),
    script!(32, "ACT_VAR_INCREASE", 2, ["VariableID", "Value", ""]),
    script!(33, "ACT_VAR_DISPLAY", 3, ["VariableID", "Value", "TextID"]),
    script!(34, "ACT_SHOW_SKIPPING_MESSAGE", 1, ["Visible", "", ""]),
    script!(35, "ACT_TROOP_ANNIHILATED", 1, ["TroopID", "", ""]),
    script!(38, "ACT_OPEN_SESAME", 1, ["PropID", "", ""]),
    script!(39, "ACT_CLOSE_SESAME", 1, ["PropID", "", ""]),
    script!(47, "ACT_EVENT_ANCIENT_HEART_CALLED_ME", 0, ["", "", ""]),
    script!(49, "ACT_MISSION_COMPLETE", 0, ["", "", ""]),
    script!(50, "ACT_MISSION_FAIL", 0, ["", "", ""]),
    script!(51, "ACT_DELAY_TICK", 1, ["Ticks", "", ""]),
    script!(52, "ACT_LOOP", 0, ["", "", ""]),
    script!(53, "ACT_RESET_TRIGGER", 0, ["", "", ""]),
    script!(54, "ACT_SHOW_TEXT", 1, ["TextID", "", ""]),
    script!(55, "ACT_VAR_INT_SET", 2, ["VariableID", "Value", ""]),
    script!(56, "ACT_VAR_RANDOM_SET", 2, ["VariableID", "Value", ""]),
    script!(57, "ACT_CAM_RESET", 0, ["", "", ""]),
    script!(58, "ACT_LETTER_BOX_ENABLE", 0, ["", "", ""]),
    script!(59, "ACT_LETTER_BOX_DISABLE", 0, ["", "", ""]),
    script!(60, "ACT_SHOW_TEXT_EX", 2, ["TextID", "Duration", ""]),
    script!(61, "ACT_RESET_TRIGGER_EX", 1, ["TriggerID", "", ""]),
    script!(62, "ACT_TROOP_SIGNAL", 2, ["TroopID", "AnimationID", ""]),
    script!(63, "ACT_BLOCK_AREA", 1, ["AreaID", "", ""]),
    script!(64, "ACT_OPEN_AREA", 1, ["AreaID", "", ""]),
    script!(65, "ACT_RECOVER_AREA", 1, ["AreaID", "", ""]),
    script!(66, "ACT_SET_AI", 2, ["TroopID", "AIID", ""]),
    script!(67, "ACT_ENABLE_AI", 1, ["TroopID", "", ""]),
    script!(68, "ACT_DISABLE_AI", 1, ["TroopID", "", ""]),
    script!(70, "ACT_SHOW_TEXT_XY_2", 3, ["X", "Y", "Duration"]),
    script!(71, "ACT_SET_SNOW", 1, ["Amount", "", ""]),
    script!(72, "ACT_REMOVE_SNOW", 0, ["", "", ""]),
    script!(73, "ACT_SET_CAM_TARGET", 1, ["TroopID", "", ""]),
    script!(74, "ACT_UNSET_CAM_TARGET", 0, ["", "", ""]),
    script!(75, "ACT_RENEW_TROOP", 1, ["TroopID", "", ""]),
    script!(76, "ACT_SHOW_TITLE", 3, ["X", "Y", "Duration"]),
    script!(77, "ACT_SHOW_TEXT_XY", 3, ["X", "Y", "Duration"]),
    script!(78, "ACT_SET_FPS", 1, ["FPS", "", ""]),
    script!(79, "ACT_RESET_FPS", 0, ["", "", ""]),
    script!(80, "ACT_SET_MOTION_BLUR", 1, ["Amount", "", ""]),
    script!(81, "ACT_RESET_MOTION_BLUR", 0, ["", "", ""]),
    script!(82, "ACT_TROOP_SET_SPEED", 2, ["TroopID", "Speed", ""]),
    script!(83, "ACT_TROOP_RESET_SPEED", 1, ["TroopID", "", ""]),
    script!(84, "ACT_SET_RAIN", 1, ["Amount", "", ""]),
    script!(85, "ACT_STOP_RAIN", 0, ["", "", ""]),
    script!(86, "ACT_SET_WIND", 2, ["Direction", "Strength", ""]),
    script!(87, "ACT_SET_GATE", 3, ["AreaID", "PlayerID", "Enable"]),
    script!(88, "ACT_START_WATER_ATTACK", 0, ["", "", ""]),
    script!(89, "ACT_CHAR_SAY_EX", 3, ["CharID", "TextID", "Duration"]),
    script!(90, "ACT_LEADER_INVULNERABLE", 1, ["TroopID", "", ""]),
    script!(91, "ACT_LEADER_VULNERABLE", 1, ["TroopID", "", ""]),
    script!(92, "ACT_LEADER_RECHARE_RATE", 2, ["TroopID", "Rate", ""]),
    script!(93, "ACT_TROOP_SET_BOUNDARY", 2, ["TroopID", "AreaID", ""]),
    script!(94, "ACT_TROOP_RESET_BOUNDARY", 1, ["TroopID", "", ""]),
    script!(95, "ACT_TROOP_WARP", 3, ["TroopID", "AreaID", "Direction"]),
    script!(96, "ACT_MY_PLAYER_GET_EXP", 1, ["EXP", "", ""]),
    script!(97, "ACT_SHOW_AREA_ON_MINIMAP", 1, ["AreaID", "", ""]),
    script!(98, "ACT_HIDE_AREA_ON_MINIMAP", 1, ["AreaID", "", ""]),
    script!(
        99,
        "ACT_SHOW_TEXT_ON_MSG_WINDOW",
        3,
        ["TextID", "Duration", "Sound"]
    ),
    script!(100, "ACT_TROOP_SIGNAL_ARROW", 2, ["TroopID", "AreaID", ""]),
    script!(101, "ACT_FADE_IN", 1, ["Duration", "", ""]),
    script!(102, "ACT_FADE_OUT", 1, ["Duration", "", ""]),
    script!(103, "ACT_OPEN_DAM", 0, ["", "", ""]),
    script!(104, "ACT_CLOSE_DAM", 0, ["", "", ""]),
    script!(
        105,
        "ACT_DISABLE_TROOPS_INSIDE_AREA",
        2,
        ["AreaID", "PlayerID", ""]
    ),
    script!(
        106,
        "ACT_DISABLE_TROOPS_OUTSIDE_AREA",
        2,
        ["AreaID", "PlayerID", ""]
    ),
    script!(107, "ACT_DISABLE_ALL_TROOPS", 0, ["", "", ""]),
    script!(
        108,
        "ACT_SET_DUMMY_TROOP_OBSTACLE_RADIUS",
        2,
        ["Index", "Radius", ""]
    ),
    script!(109, "ACT_COLLAPSE_WALL", 1, ["Index", "", ""]),
    script!(110, "ACT_PLAY_BGM", 3, ["Filename", "BGMID", "Repeat"]),
    script!(111, "ACT_STOP_BGM", 1, ["BGMID", "", ""]),
    script!(
        112,
        "ACT_START_TROOP_INDICATE_IN_MINIMAP",
        1,
        ["TroopID", "", ""]
    ),
    script!(
        113,
        "ACT_STOP_TROOP_INDICATE_IN_MINIMAP",
        1,
        ["TroopID", "", ""]
    ),
    script!(114, "ACT_TROOP_REFILL_HP", 1, ["TroopID", "", ""]),
    script!(115, "ACT_TROOP_SET_HP", 2, ["TroopID", "Percent", ""]),
    script!(116, "ACT_LIP_SYNC_BEGIN", 2, ["TroopID", "Slot", ""]),
    script!(117, "ACT_LIP_SYNC_END", 2, ["TroopID", "Slot", ""]),
    script!(118, "ACT_SET_FIRE_SPREAD_SPEED", 1, ["Speed", "", ""]),
    script!(119, "ACT_ENABLE_INPUT", 0, ["", "", ""]),
    script!(120, "ACT_DISABLE_INPUT", 0, ["", "", ""]),
    script!(121, "ACT_ENABLE_FOG_OF_WAR", 1, ["Enable", "", ""]),
    script!(122, "ACT_SET_FIRE_SPREAD_RANGE", 1, ["Range", "", ""]),
    script!(123, "ACT_SET_FIRE", 1, ["AreaID", "", ""]),
    script!(124, "ACT_REGNIER_GO_CRAZY", 1, ["TroopID", "", ""]),
    script!(125, "ACT_REGNIER_FREE_HIS_POWER", 1, ["TroopID", "", ""]),
    script!(126, "ACT_SET_MUTE", 1, ["Mute", "", ""]),
    script!(127, "ACT_BURY_TROOP", 1, ["TroopID", "", ""]),
    script!(128, "ACT_OFFICER_SAY", 2, ["CharID", "TextID", ""]),
    script!(129, "ACT_TAG_THE_TROOP", 2, ["TroopID", "Tag", ""]),
    script!(130, "ACT_UNTAG_THE_TROOP", 1, ["TroopID", "", ""]),
    script!(
        131,
        "ACT_SHOW_VAR_GAUGE",
        3,
        ["PortraitID", "VariableID", "TextID"]
    ),
    script!(132, "ACT_HIDE_VAR_GAUGE", 0, ["", "", ""]),
    script!(
        133,
        "ACT_TROOP_ANIMATION",
        2,
        ["TroopID", "AnimationID", ""]
    ),
    script!(
        135,
        "ACT_TROOP_RANGE_ATTACK_ON_POS",
        2,
        ["TroopID", "AreaID", ""]
    ),
    script!(
        136,
        "ACT_TROOP_RANGE_ATTACK_ON_PROP",
        2,
        ["TroopID", "PropID", ""]
    ),
    script!(137, "ACT_SET_AI_PATH", 2, ["PathID", "AreaID", ""]),
    script!(138, "ACT_ENABLE_ABILITY", 1, ["AbilityID", "", ""]),
    script!(139, "ACT_DISABLE_ABILITY", 1, ["AbilityID", "", ""]),
    script!(140, "ACT_SHOW_NOISE_METER_GAUGE", 1, ["TextID", "", ""]),
    script!(141, "ACT_SET_BGM_VOLUME", 2, ["BGMID", "Percent", ""]),
    script!(142, "ACT_FADE_BGM", 3, ["BGMID", "Ticks", "Percent"]),
    script!(
        143,
        "ACT_MARK_ON_TROOP_IN_AREA",
        3,
        ["AreaID", "PlayerID", "Ticks"]
    ),
    script!(144, "ACT_SET_WALL_HP", 2, ["PropID", "HP", ""]),
    script!(145, "ACT_TROOP_SET_INVULNERABLE", 1, ["TroopID", "", ""]),
    script!(146, "ACT_TROOP_RESET_INVULNERABLE", 1, ["TroopID", "", ""]),
    script!(147, "ACT_TROOP_SELECT", 1, ["TroopID", "", ""]),
    script!(148, "ACT_FORCE_MINIMAP_ON", 0, ["", "", ""]),
    script!(149, "ACT_FORCE_MINIMAP_OFF", 0, ["", "", ""]),
    script!(150, "ACT_SHOW_PROP_HP_GAUGE", 2, ["TextID", "PropID", ""]),
    script!(151, "ACT_SHOW_TROOP_HP_GAUGE", 2, ["TextID", "TroopID", ""]),
    script!(
        152,
        "ACT_RENEW_TROOP_OUTOFSIGHT",
        3,
        ["TroopID", "AreaID", "PlayerID"]
    ),
    script!(153, "ACT_SET_TRAP", 3, ["AreaID", "PlayerID", "Count"]),
    script!(154, "ACT_SET_MINE", 3, ["AreaID", "PlayerID", "Count"]),
    script!(155, "ACT_FLOOD_RESET", 0, ["", "", ""]),
    script!(157, "ACT_ENABLE_PAUSE", 0, ["", "", ""]),
    script!(158, "ACT_DISABLE_PAUSE", 0, ["", "", ""]),
    script!(159, "ACT_QUICK_SAVE", 0, ["", "", ""]),
    script!(160, "ACT_CHAR_RANDOM_SAY", 2, ["PortraitID", "TextID", ""]),
    script!(
        161,
        "ACT_ENABLE_TROOP_IN_AREA",
        2,
        ["AreaID", "PlayerID", ""]
    ),
    script!(162, "ACT_PLAYER_TROOP_STOP", 1, ["PlayerID", "", ""]),
    script!(163, "ACT_EXCLUSIVE_BEGIN", 0, ["", "", ""]),
    script!(164, "ACT_EXCLUSIVE_END", 0, ["", "", ""]),
    script!(165, "ACT_LOAD_MISSION", 2, ["Filename", "Mode", ""]),
    script!(166, "ACT_JOYPAD_RUMBLE", 1, ["Enable", "", ""]),
    script!(
        167,
        "ACT_SHOW_TROOP_DIST_GAUGE",
        2,
        ["PortraitID", "TextID", ""]
    ),
    script!(168, "ACT_SET_FIRE_N_SMOKE", 2, ["AreaID", "Count", ""]),
    script!(169, "ACT_SET_WATER_EFFECT_PROP", 1, ["AreaID", "", ""]),
    script!(170, "ACT_SET_SCREEN_GLOW", 1, ["Enable", "", ""]),
    script!(
        171,
        "ACT_UPDATE_UNIT_KILL_COUNT",
        2,
        ["VariableID", "PlayerID", ""]
    ),
    script!(172, "ACT_PLAY_FMV", 2, ["Filename", "NextMission", ""]),
    script!(173, "ACT_ENABLE_LENS_FLARE", 1, ["Enable", "", ""]),
    script!(
        174,
        "ACT_CHANGE_SKYBOX_N_LIGHT_SET",
        2,
        ["Skybox", "LightSet", ""]
    ),
    script!(175, "ACT_REMOVE_TRAP", 2, ["AreaID", "PlayerID", ""]),
    script!(
        176,
        "ACT_SET_FIRE_N_SMOKE_SMALL",
        2,
        ["AreaID", "Count", ""]
    ),
    script!(177, "ACT_SET_TRAINING_MISSION", 2, ["Slot", "Enable", ""]),
    script!(178, "ACT_SET_LIBRARY", 2, ["Slot", "Enable", ""]),
    script!(179, "ACT_ALL_MISSION_COMPLETE", 0, ["", "", ""]),
    script!(
        180,
        "ACT_PLAY_FMV_AND_GO_TO_WORLDMAP",
        1,
        ["Filename", "", ""]
    ),
    script!(181, "ACT_GO_TO_WORLDMAP", 0, ["", "", ""]),
    script!(182, "ACT_SKIP_TEXT", 0, ["", "", ""]),
];
