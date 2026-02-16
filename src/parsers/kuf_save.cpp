// This is a generated file! Please edit source .ksy file and use kaitai-struct-compiler to rebuild

#include "kuf_save.h"
#include "kaitai/exceptions.h"

kuf_save_t::kuf_save_t(kaitai::kstream* p__io, kaitai::kstruct* p__parent, kuf_save_t* p__root) : kaitai::kstruct(p__io) {
    m__parent = p__parent;
    m__root = p__root ? p__root : this;
    m_units = 0;
    m_roster_entries = 0;
    m_second_array = 0;
    m_mission_completion = 0;

    try {
        _read();
    } catch(...) {
        _clean_up();
        throw;
    }
}

void kuf_save_t::_read() {
    m_size_prefix = m__io->read_u4le();
    {
        uint32_t _ = m_size_prefix;
        if (!(_ == _io()->size())) {
            throw kaitai::validation_expr_error<uint32_t>(m_size_prefix, m__io, std::string("/seq/0"));
        }
    }
    m_magic = m__io->read_u4le();
    if (!(m_magic == 110)) {
        throw kaitai::validation_not_equal_error<uint32_t>(110, m_magic, m__io, std::string("/seq/1"));
    }
    m_context_data = m__io->read_bytes(1080);
    m_campaign_index = m__io->read_u4le();
    if (!(m_campaign_index <= 3)) {
        throw kaitai::validation_greater_than_error<uint32_t>(3, m_campaign_index, m__io, std::string("/seq/3"));
    }
    m_main_save_block = m__io->read_bytes(340);
    m_unit_count = m__io->read_u4le();
    m_units = new std::vector<unit_save_data_t*>();
    const int l_units = unit_count();
    for (int i = 0; i < l_units; i++) {
        m_units->push_back(new unit_save_data_t(m__io, this, m__root));
    }
    m_selected_unit_ref = m__io->read_u4le();
    m_roster_count = m__io->read_u4le();
    m_roster_entries = new std::vector<roster_entry_t*>();
    const int l_roster_entries = roster_count();
    for (int i = 0; i < l_roster_entries; i++) {
        m_roster_entries->push_back(new roster_entry_t(m__io, this, m__root));
    }
    m_second_array_count = m__io->read_u4le();
    m_second_array = new std::vector<uint32_t>();
    const int l_second_array = second_array_count();
    for (int i = 0; i < l_second_array; i++) {
        m_second_array->push_back(m__io->read_u4le());
    }
    m_mission_completion = new std::vector<uint32_t>();
    const int l_mission_completion = 20;
    for (int i = 0; i < l_mission_completion; i++) {
        m_mission_completion->push_back(m__io->read_u4le());
    }
    m_current_mission_slot = m__io->read_u4le();
    m_tail_data = m__io->read_bytes_full();
}

kuf_save_t::~kuf_save_t() {
    _clean_up();
}

void kuf_save_t::_clean_up() {
    if (m_units) {
        for (std::vector<unit_save_data_t*>::iterator it = m_units->begin(); it != m_units->end(); ++it) {
            delete *it;
        }
        delete m_units; m_units = 0;
    }
    if (m_roster_entries) {
        for (std::vector<roster_entry_t*>::iterator it = m_roster_entries->begin(); it != m_roster_entries->end(); ++it) {
            delete *it;
        }
        delete m_roster_entries; m_roster_entries = 0;
    }
    if (m_second_array) {
        delete m_second_array; m_second_array = 0;
    }
    if (m_mission_completion) {
        delete m_mission_completion; m_mission_completion = 0;
    }
}

kuf_save_t::roster_entry_t::roster_entry_t(kaitai::kstream* p__io, kuf_save_t* p__parent, kuf_save_t* p__root) : kaitai::kstruct(p__io) {
    m__parent = p__parent;
    m__root = p__root;

    try {
        _read();
    } catch(...) {
        _clean_up();
        throw;
    }
}

void kuf_save_t::roster_entry_t::_read() {
    m_byte_61 = m__io->read_u1();
    m_byte_60 = m__io->read_u1();
    m_byte_62 = m__io->read_u1();
    m_byte_63 = m__io->read_u1();
    m_uint_64 = m__io->read_u4le();
}

kuf_save_t::roster_entry_t::~roster_entry_t() {
    _clean_up();
}

void kuf_save_t::roster_entry_t::_clean_up() {
}

kuf_save_t::unit_save_data_t::unit_save_data_t(kaitai::kstream* p__io, kuf_save_t* p__parent, kuf_save_t* p__root) : kaitai::kstruct(p__io) {
    m__parent = p__parent;
    m__root = p__root;

    try {
        _read();
    } catch(...) {
        _clean_up();
        throw;
    }
}

void kuf_save_t::unit_save_data_t::_read() {
    m_unknown_index = m__io->read_u4le();
    m_troop_info_index = m__io->read_u4le();
    m_job_type = m__io->read_u4le();
    m_model_id = m__io->read_u4le();
    m_stg_field_190 = m__io->read_u4le();
    m_stg_field_192 = m__io->read_u4le();
    m_stg_field_194 = m__io->read_u4le();
    m_stg_field_198 = m__io->read_u4le();
    m_char_id = m__io->read_u4le();
    m_troop_info_index_2 = m__io->read_u4le();
    m_ucd = m__io->read_u4le();
    m_formation_type = m__io->read_u4le();
    m_grid_config = m__io->read_u4le();
    m_skill_level = m__io->read_u4le();
    m_byte_58 = m__io->read_u1();
    m_hero_flag = m__io->read_u1();
    m_byte_5a = m__io->read_u1();
    m_field_60 = m__io->read_u4le();
    m_field_64 = m__io->read_u4le();
    m_field_68 = m__io->read_u4le();
    m_equipment = m__io->read_bytes(24);
    m_leader_abilities_1 = m__io->read_bytes(64);
    m_officer1_abilities_1 = m__io->read_bytes(64);
    m_officer2_abilities_1 = m__io->read_bytes(64);
    m_leader_abilities_2 = m__io->read_bytes(64);
    m_officer1_abilities_2 = m__io->read_bytes(64);
    m_officer2_abilities_2 = m__io->read_bytes(64);
    m_field_504 = m__io->read_u4le();
}

kuf_save_t::unit_save_data_t::~unit_save_data_t() {
    _clean_up();
}

void kuf_save_t::unit_save_data_t::_clean_up() {
}
