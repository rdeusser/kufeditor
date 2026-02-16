#ifndef KUF_SAVE_H_
#define KUF_SAVE_H_

// This is a generated file! Please edit source .ksy file and use kaitai-struct-compiler to rebuild

class kuf_save_t;

#include "kaitai/kaitaistruct.h"
#include <stdint.h>
#include <vector>

#if KAITAI_STRUCT_VERSION < 11000L
#error "Incompatible Kaitai Struct C++/STL API: version 0.11 or later is required"
#endif

/**
 * Save file format for Kingdom Under Fire: The Crusaders (PC port).
 * Saves are created from the world map between missions.
 * The file uses a 4-byte size prefix (Xbox memory card remnant)
 * and is zero-padded to 32KB (0x8000 bytes of stream data).
 */

class kuf_save_t : public kaitai::kstruct {

public:
    class roster_entry_t;
    class unit_save_data_t;

    kuf_save_t(kaitai::kstream* p__io, kaitai::kstruct* p__parent = 0, kuf_save_t* p__root = 0);

private:
    void _read();
    void _clean_up();

public:
    ~kuf_save_t();

    class roster_entry_t : public kaitai::kstruct {

    public:

        roster_entry_t(kaitai::kstream* p__io, kuf_save_t* p__parent = 0, kuf_save_t* p__root = 0);

    private:
        void _read();
        void _clean_up();

    public:
        ~roster_entry_t();

    private:
        uint8_t m_byte_61;
        uint8_t m_byte_60;
        uint8_t m_byte_62;
        uint8_t m_byte_63;
        uint32_t m_uint_64;
        kuf_save_t* m__root;
        kuf_save_t* m__parent;

    public:
        uint8_t byte_61() const { return m_byte_61; }
        uint8_t byte_60() const { return m_byte_60; }
        uint8_t byte_62() const { return m_byte_62; }
        uint8_t byte_63() const { return m_byte_63; }
        uint32_t uint_64() const { return m_uint_64; }
        kuf_save_t* _root() const { return m__root; }
        kuf_save_t* _parent() const { return m__parent; }
    };

    /**
     * Per-unit save data. 483 bytes per unit.
     */

    class unit_save_data_t : public kaitai::kstruct {

    public:

        unit_save_data_t(kaitai::kstream* p__io, kuf_save_t* p__parent = 0, kuf_save_t* p__root = 0);

    private:
        void _read();
        void _clean_up();

    public:
        ~unit_save_data_t();

    private:
        uint32_t m_unknown_index;
        uint32_t m_troop_info_index;
        uint32_t m_job_type;
        uint32_t m_model_id;
        uint32_t m_stg_field_190;
        uint32_t m_stg_field_192;
        uint32_t m_stg_field_194;
        uint32_t m_stg_field_198;
        uint32_t m_char_id;
        uint32_t m_troop_info_index_2;
        uint32_t m_ucd;
        uint32_t m_formation_type;
        uint32_t m_grid_config;
        uint32_t m_skill_level;
        uint8_t m_byte_58;
        uint8_t m_hero_flag;
        uint8_t m_byte_5a;
        uint32_t m_field_60;
        uint32_t m_field_64;
        uint32_t m_field_68;
        std::string m_equipment;
        std::string m_leader_abilities_1;
        std::string m_officer1_abilities_1;
        std::string m_officer2_abilities_1;
        std::string m_leader_abilities_2;
        std::string m_officer1_abilities_2;
        std::string m_officer2_abilities_2;
        uint32_t m_field_504;
        kuf_save_t* m__root;
        kuf_save_t* m__parent;

    public:

        /**
         * Runtime 0x24, default -1
         */
        uint32_t unknown_index() const { return m_unknown_index; }

        /**
         * Runtime 0x28
         */
        uint32_t troop_info_index() const { return m_troop_info_index; }

        /**
         * Runtime 0x2C. K2_JOB_TYPE enum or CharInfo index (>=43)
         */
        uint32_t job_type() const { return m_job_type; }

        /**
         * Runtime 0x30, sub_type
         */
        uint32_t model_id() const { return m_model_id; }

        /**
         * Runtime 0x34
         */
        uint32_t stg_field_190() const { return m_stg_field_190; }

        /**
         * Runtime 0x38
         */
        uint32_t stg_field_192() const { return m_stg_field_192; }

        /**
         * Runtime 0x3C
         */
        uint32_t stg_field_194() const { return m_stg_field_194; }

        /**
         * Runtime 0x40
         */
        uint32_t stg_field_198() const { return m_stg_field_198; }

        /**
         * Runtime 0x20, unit identity from STG byte 0x56
         */
        uint32_t char_id() const { return m_char_id; }

        /**
         * Runtime 0x48, from STG offset 0x1C0
         */
        uint32_t troop_info_index_2() const { return m_troop_info_index_2; }

        /**
         * Runtime 0x4C. 0=Player, 1=Enemy, 2=Ally, 3=Neutral
         */
        uint32_t ucd() const { return m_ucd; }

        /**
         * Runtime 0x44 (written out of sequential order)
         */
        uint32_t formation_type() const { return m_formation_type; }

        /**
         * Runtime 0x50
         */
        uint32_t grid_config() const { return m_grid_config; }

        /**
         * Runtime 0x54, computed by CalcUnitSkillLevel
         */
        uint32_t skill_level() const { return m_skill_level; }

        /**
         * Runtime 0x58, default 0x01
         */
        uint8_t byte_58() const { return m_byte_58; }

        /**
         * Runtime 0x59. 0=hero, 1=troop
         */
        uint8_t hero_flag() const { return m_hero_flag; }

        /**
         * Runtime 0x5A, default 0x01
         */
        uint8_t byte_5a() const { return m_byte_5a; }
        uint32_t field_60() const { return m_field_60; }
        uint32_t field_64() const { return m_field_64; }
        uint32_t field_68() const { return m_field_68; }

        /**
         * 6 equipment slots, 4 bytes each
         */
        std::string equipment() const { return m_equipment; }
        std::string leader_abilities_1() const { return m_leader_abilities_1; }
        std::string officer1_abilities_1() const { return m_officer1_abilities_1; }
        std::string officer2_abilities_1() const { return m_officer2_abilities_1; }
        std::string leader_abilities_2() const { return m_leader_abilities_2; }
        std::string officer1_abilities_2() const { return m_officer1_abilities_2; }
        std::string officer2_abilities_2() const { return m_officer2_abilities_2; }
        uint32_t field_504() const { return m_field_504; }
        kuf_save_t* _root() const { return m__root; }
        kuf_save_t* _parent() const { return m__parent; }
    };

private:
    uint32_t m_size_prefix;
    uint32_t m_magic;
    std::string m_context_data;
    uint32_t m_campaign_index;
    std::string m_main_save_block;
    uint32_t m_unit_count;
    std::vector<unit_save_data_t*>* m_units;
    uint32_t m_selected_unit_ref;
    uint32_t m_roster_count;
    std::vector<roster_entry_t*>* m_roster_entries;
    uint32_t m_second_array_count;
    std::vector<uint32_t>* m_second_array;
    std::vector<uint32_t>* m_mission_completion;
    uint32_t m_current_mission_slot;
    std::string m_tail_data;
    kuf_save_t* m__root;
    kaitai::kstruct* m__parent;

public:

    /**
     * File size in bytes. Equals total file size (memory card format remnant).
     */
    uint32_t size_prefix() const { return m_size_prefix; }

    /**
     * Save format magic number (0x6E = 110)
     */
    uint32_t magic() const { return m_magic; }

    /**
     * Save context (1080 bytes): slot name, timestamp, display strings
     */
    std::string context_data() const { return m_context_data; }

    /**
     * 0=Hironeiden/Gerald, 1=Vellond/Lucretia, 2=Ecclesia/Kendal, 3=Dark Legion/Regnier
     */
    uint32_t campaign_index() const { return m_campaign_index; }

    /**
     * Main save data block (340 bytes from game state object at +0xA4)
     */
    std::string main_save_block() const { return m_main_save_block; }
    uint32_t unit_count() const { return m_unit_count; }
    std::vector<unit_save_data_t*>* units() const { return m_units; }

    /**
     * Index of selected/active unit
     */
    uint32_t selected_unit_ref() const { return m_selected_unit_ref; }
    uint32_t roster_count() const { return m_roster_count; }
    std::vector<roster_entry_t*>* roster_entries() const { return m_roster_entries; }
    uint32_t second_array_count() const { return m_second_array_count; }

    /**
     * Hash-map key references
     */
    std::vector<uint32_t>* second_array() const { return m_second_array; }

    /**
     * Mission completion flags (20 entries from DAT_00743960, 0x4C stride)
     */
    std::vector<uint32_t>* mission_completion() const { return m_mission_completion; }
    uint32_t current_mission_slot() const { return m_current_mission_slot; }

    /**
     * Remaining data: mission state (variable-length, NOT self-describing),
     * campaign-specific data (5 or 50 bytes), script object array, zero padding.
     * Cannot be structurally parsed without runtime STG context.
     */
    std::string tail_data() const { return m_tail_data; }
    kuf_save_t* _root() const { return m__root; }
    kaitai::kstruct* _parent() const { return m__parent; }
};

#endif  // KUF_SAVE_H_
