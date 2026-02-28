#pragma once

#include <cstdint>

namespace kuf {

// Encodes object type + block/event/entry/sub indices into 64-bit IDs
// for imgui-node-editor. Layout:
//
//   Bits 63-60: Type (EventGroup=1, Condition=2, Action=3, Pin=4, Link=5)
//   Bits 59-48: Block index  (12 bits, max 4095)
//   Bits 47-32: Event index  (16 bits)
//   Bits 31-16: Entry index  (16 bits)
//   Bits 15-0:  Sub-index    (16 bits, pin slot or link pair)

enum class NodeType : uint64_t {
	EventGroup = 1,
	Condition = 2,
	Action = 3,
	Pin = 4,
	Link = 5,
};

constexpr uint64_t encodeId(NodeType type, uint64_t block, uint64_t event,
			    uint64_t entry, uint64_t sub) {
	return (static_cast<uint64_t>(type) << 60) | ((block & 0xFFF) << 48) |
	       ((event & 0xFFFF) << 32) | ((entry & 0xFFFF) << 16) |
	       (sub & 0xFFFF);
}

constexpr NodeType decodeType(uint64_t id) {
	return static_cast<NodeType>((id >> 60) & 0xF);
}

constexpr uint64_t decodeBlock(uint64_t id) { return (id >> 48) & 0xFFF; }
constexpr uint64_t decodeEvent(uint64_t id) { return (id >> 32) & 0xFFFF; }
constexpr uint64_t decodeEntry(uint64_t id) { return (id >> 16) & 0xFFFF; }
constexpr uint64_t decodeSub(uint64_t id) { return id & 0xFFFF; }

constexpr uint64_t eventGroupNode(uint64_t block, uint64_t event) {
	return encodeId(NodeType::EventGroup, block, event, 0, 0);
}

constexpr uint64_t conditionNode(uint64_t block, uint64_t event,
				 uint64_t entry) {
	return encodeId(NodeType::Condition, block, event, entry, 0);
}

constexpr uint64_t actionNode(uint64_t block, uint64_t event, uint64_t entry) {
	return encodeId(NodeType::Action, block, event, entry, 0);
}

// Output pin on a condition node (right side).
constexpr uint64_t conditionOutputPin(uint64_t block, uint64_t event,
				      uint64_t entry) {
	return encodeId(NodeType::Pin, block, event, entry, 0);
}

// Input pin on an action node (left side).
constexpr uint64_t actionInputPin(uint64_t block, uint64_t event,
				  uint64_t entry) {
	return encodeId(NodeType::Pin, block, event, entry, 1);
}

// Link from condition condIdx to action actIdx within an event.
constexpr uint64_t eventLink(uint64_t block, uint64_t event, uint64_t condIdx,
			     uint64_t actIdx) {
	return encodeId(NodeType::Link, block, event, condIdx, actIdx);
}

} // namespace kuf
