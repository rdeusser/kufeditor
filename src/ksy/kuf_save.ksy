meta:
  id: kuf_save
  title: Kingdom Under Fire Crusaders Save File
  file-extension: sav
  endian: le
  bit-endian: le

doc: |
  Save file format for Kingdom Under Fire: The Crusaders (PC port).
  Saves are created from the world map between missions.
  The file uses a 4-byte size prefix (Xbox memory card remnant)
  and is zero-padded to 32KB (0x8000 bytes of stream data).

seq:
  - id: size_prefix
    type: u4
    doc: "File size in bytes. Equals total file size (memory card format remnant)."
    valid:
      expr: _ == _io.size

  - id: magic
    type: u4
    valid:
      eq: 0x6e
    doc: "Save format magic number (0x6E = 110)"

  - id: context_data
    size: 0x438
    doc: "Save context (1080 bytes): slot name, timestamp, display strings"

  - id: campaign_index
    type: u4
    doc: "0=Hironeiden/Gerald, 1=Vellond/Lucretia, 2=Ecclesia/Kendal, 3=Dark Legion/Regnier"
    valid:
      max: 3

  - id: main_save_block
    size: 0x154
    doc: "Main save data block (340 bytes from game state object at +0xA4)"

  - id: unit_count
    type: u4

  - id: units
    type: unit_save_data
    repeat: expr
    repeat-expr: unit_count

  - id: selected_unit_ref
    type: u4
    doc: "Index of selected/active unit"

  - id: roster_count
    type: u4

  - id: roster_entries
    type: roster_entry
    repeat: expr
    repeat-expr: roster_count

  - id: second_array_count
    type: u4

  - id: second_array
    type: u4
    repeat: expr
    repeat-expr: second_array_count
    doc: "Hash-map key references"

  - id: mission_completion
    type: u4
    repeat: expr
    repeat-expr: 20
    doc: "Mission completion flags (20 entries from DAT_00743960, 0x4C stride)"

  - id: current_mission_slot
    type: u4

  - id: tail_data
    size-eos: true
    doc: |
      Remaining data: mission state (variable-length, NOT self-describing),
      campaign-specific data (5 or 50 bytes), script object array, zero padding.
      Cannot be structurally parsed without runtime STG context.

types:
  unit_save_data:
    doc: "Per-unit save data. 483 bytes per unit."
    seq:
      - id: unknown_index
        type: u4
        doc: "Runtime 0x24, default -1"
      - id: troop_info_index
        type: u4
        doc: "Runtime 0x28"
      - id: job_type
        type: u4
        doc: "Runtime 0x2C. K2_JOB_TYPE enum or CharInfo index (>=43)"
      - id: model_id
        type: u4
        doc: "Runtime 0x30, sub_type"
      - id: stg_field_190
        type: u4
        doc: "Runtime 0x34"
      - id: stg_field_192
        type: u4
        doc: "Runtime 0x38"
      - id: stg_field_194
        type: u4
        doc: "Runtime 0x3C"
      - id: stg_field_198
        type: u4
        doc: "Runtime 0x40"
      - id: char_id
        type: u4
        doc: "Runtime 0x20, unit identity from STG byte 0x56"
      - id: troop_info_index_2
        type: u4
        doc: "Runtime 0x48, from STG offset 0x1C0"
      - id: ucd
        type: u4
        doc: "Runtime 0x4C. 0=Player, 1=Enemy, 2=Ally, 3=Neutral"
      - id: formation_type
        type: u4
        doc: "Runtime 0x44 (written out of sequential order)"
      - id: grid_config
        type: u4
        doc: "Runtime 0x50"
      - id: skill_level
        type: u4
        doc: "Runtime 0x54, computed by CalcUnitSkillLevel"
      - id: byte_58
        type: u1
        doc: "Runtime 0x58, default 0x01"
      - id: hero_flag
        type: u1
        doc: "Runtime 0x59. 0=hero, 1=troop"
      - id: byte_5a
        type: u1
        doc: "Runtime 0x5A, default 0x01"
      - id: field_60
        type: u4
      - id: field_64
        type: u4
      - id: field_68
        type: u4
      - id: equipment
        size: 24
        doc: "6 equipment slots, 4 bytes each"
      - id: leader_abilities_1
        size: 64
      - id: officer1_abilities_1
        size: 64
      - id: officer2_abilities_1
        size: 64
      - id: leader_abilities_2
        size: 64
      - id: officer1_abilities_2
        size: 64
      - id: officer2_abilities_2
        size: 64
      - id: field_504
        type: u4

  roster_entry:
    seq:
      - id: byte_61
        type: u1
      - id: byte_60
        type: u1
      - id: byte_62
        type: u1
      - id: byte_63
        type: u1
      - id: uint_64
        type: u4
