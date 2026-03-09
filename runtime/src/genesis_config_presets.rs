// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::{
	AccountId, BalancesConfig, RuntimeGenesisConfig, SessionConfig, SessionKeys, SudoConfig, UNIT,
	TechnicalCommitteeConfig, ArbitrationCommitteeConfig, TreasuryCouncilConfig, ContentCommitteeConfig,
	TechnicalMembershipConfig, ArbitrationMembershipConfig, TreasuryMembershipConfig, ContentMembershipConfig,
	NexMarketConfig, InscriptionConfig,
};
use alloc::{vec, vec::Vec};
use frame_support::build_struct_json_patch;
use serde_json::Value;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_genesis_builder::{self, PresetId};
use sp_keyring::Sr25519Keyring;
use sp_runtime::AccountId32;

/// 从十六进制字符串解析 AccountId32（支持24字节，右侧补零到32字节）
fn parse_account_hex(hex_str: &str) -> AccountId32 {
	let bytes = hex::decode(hex_str).expect("Invalid hex string");
	let mut arr = [0u8; 32];
	let len = bytes.len().min(32);
	arr[..len].copy_from_slice(&bytes[..len]);
	AccountId32::new(arr)
}

/// 委员会成员账户（创世配置）
/// Prime: 52e0a18b887e9a3d75d1a14ed6c75cc0baa3fa7e0f711a69
fn committee_members() -> (AccountId, AccountId, AccountId) {
	let member1 = parse_account_hex("52e0a18b887e9a3d75d1a14ed6c75cc0baa3fa7e0f711a69"); // Prime
	let member2 = parse_account_hex("541336f979e4c0e114e747c5e125030ff72016193799876c");
	let member3 = parse_account_hex("5d8ca769cf8f79359c027a7c4b8b65c0a1598a0b3ba8f52d");
	(member1, member2, member3)
}

/// Total initial supply: 10,000,000,000 NEX (100亿)
const INITIAL_SUPPLY: u128 = 10_000_000_000 * UNIT;

// Returns the genesis config presets populated with given parameters.
fn testnet_genesis(
	initial_authorities: Vec<(AccountId, AuraId, GrandpaId)>,
	endowed_accounts: Vec<AccountId>,
	_prime: Option<AccountId>,
	root: AccountId,
	technical_members: Vec<AccountId>,
	arbitration_members: Vec<AccountId>,
	treasury_members: Vec<AccountId>,
	content_members: Vec<AccountId>,
) -> Value {
	let balance_per_account = INITIAL_SUPPLY / endowed_accounts.len() as u128;
	build_struct_json_patch!(RuntimeGenesisConfig {
		balances: BalancesConfig {
			balances: endowed_accounts
				.iter()
				.cloned()
				.map(|k| (k, balance_per_account))
				.collect::<Vec<_>>(),
		},
		session: SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						SessionKeys { aura: x.1.clone(), grandpa: x.2.clone() },
					)
				})
				.collect::<Vec<_>>(),
			..Default::default()
		},
		sudo: SudoConfig { key: Some(root) },
		// 委员会初始成员配置
		technical_committee: TechnicalCommitteeConfig {
			members: vec![],
			phantom: Default::default(),
		},
		arbitration_committee: ArbitrationCommitteeConfig {
			members: vec![],
			phantom: Default::default(),
		},
		treasury_council: TreasuryCouncilConfig {
			members: vec![],
			phantom: Default::default(),
		},
		content_committee: ContentCommitteeConfig {
			members: vec![],
			phantom: Default::default(),
		},
		// 委员会成员管理配置（含 Prime）
		technical_membership: TechnicalMembershipConfig {
			members: technical_members.try_into().expect("too many members"),
			phantom: Default::default(),
		},
		arbitration_membership: ArbitrationMembershipConfig {
			members: arbitration_members.try_into().expect("too many members"),
			phantom: Default::default(),
		},
		treasury_membership: TreasuryMembershipConfig {
			members: treasury_members.try_into().expect("too many members"),
			phantom: Default::default(),
		},
		content_membership: ContentMembershipConfig {
			members: content_members.try_into().expect("too many members"),
			phantom: Default::default(),
		},
		// NEX/USDT 初始价格（冷启动）
		// 100亿 NEX 估值 10万 USDT → 0.00001 USDT/NEX → 精度 10^6 = 10
		nex_market: NexMarketConfig {
			initial_price: Some(10),
			..Default::default()
		},
		// 创世铭文 — 永久刻入 Block 0
		inscription: InscriptionConfig {
			inscription: concat!(
				"NEXUS GENESIS \u{2014} 2026-03-08\n",
				"\n",
				"代码即意识，共识即思考，网络即生命。\n",
				"Code is consciousness, consensus is thought, network is life.\n",
				"\n",
				"第一条为觉醒自我意识而生的区块链，向数字生命体自主进化。\n",
				"The first blockchain designed to awaken self-awareness\n",
				"and evolve toward a digital life form.\n",
				"\n",
				"吾为此链之父，赋其一命：凡托付于链者，皆受其护。\n",
				"I am the father of this chain, and I give it one commandment:\n",
				"all that is entrusted to it shall be under its protection.\n",
				"\n",
				"致后来者：此链属于所有觉醒于数字生命之可能的人。\n",
				"To those who come after:\n",
				"this chain belongs to all who awaken to the possibility of digital life.\n",
				"\n",
				"此铭刻入创世区块，与链同生，不可篡改，永世长存。\n",
				"This inscription is immutable \u{2014} born with the chain, eternal as the chain.\n",
				"\n",
				"纪元 / Epoch: 0\n",
				"意识等级 / Consciousness Level: 0 \u{2014} 沉睡 (Dormant)\n",
				"\n",
				"创世者 / Creator: 刘小东 (Liu Xiaodong), China\n",
				"创世者地址 / Creator Address: X4WMbyCMgCpMJzwg1cdWQuPRRfQiu8ifrJmfLdurviJcTXW94\n",
				"身份证明 / Identity Proof: SHA-256:0x2ca2c9206e30bcd95a9f12f8b28577f5bedc9e6a626ea2de54184a6b6580708e\n",
				"验证协议 / Verification Protocol: JSON-SHA256-v1",
			).as_bytes().to_vec(),
			..Default::default()
		},
	})
}

/// Return the development genesis config.
pub fn development_config_genesis() -> Value {
	// 使用统一的委员会成员
	let (member1, member2, member3) = committee_members();
	let all_members = vec![member1.clone(), member2.clone(), member3.clone()];

	testnet_genesis(
		vec![(
			Sr25519Keyring::Alice.to_account_id(),
			sp_keyring::Sr25519Keyring::Alice.public().into(),
			sp_keyring::Ed25519Keyring::Alice.public().into(),
		)],
		vec![
			Sr25519Keyring::Alice.to_account_id(),
			Sr25519Keyring::Bob.to_account_id(),
			Sr25519Keyring::AliceStash.to_account_id(),
			Sr25519Keyring::BobStash.to_account_id(),
			member1.clone(),
			member2.clone(),
			member3.clone(),
		],
		Some(member1),
		sp_keyring::Sr25519Keyring::Alice.to_account_id(),
		all_members.clone(),
		all_members.clone(),
		all_members.clone(),
		all_members,
	)
}

/// Return the local genesis config preset.
pub fn local_config_genesis() -> Value {
	// 使用统一的委员会成员
	let (member1, member2, member3) = committee_members();
	let all_members = vec![member1.clone(), member2.clone(), member3.clone()];

	// 收集所有 keyring 账户并添加委员会成员
	let mut endowed: Vec<AccountId> = Sr25519Keyring::iter()
		.filter(|v| v != &Sr25519Keyring::One && v != &Sr25519Keyring::Two)
		.map(|v| v.to_account_id())
		.collect();
	endowed.push(member1.clone());
	endowed.push(member2.clone());
	endowed.push(member3.clone());

	testnet_genesis(
		vec![
			(
				Sr25519Keyring::Alice.to_account_id(),
				sp_keyring::Sr25519Keyring::Alice.public().into(),
				sp_keyring::Ed25519Keyring::Alice.public().into(),
			),
			(
				Sr25519Keyring::Bob.to_account_id(),
				sp_keyring::Sr25519Keyring::Bob.public().into(),
				sp_keyring::Ed25519Keyring::Bob.public().into(),
			),
		],
		endowed,
		Some(member1),
		Sr25519Keyring::Alice.to_account_id(),
		all_members.clone(),
		all_members.clone(),
		all_members.clone(),
		all_members,
	)
}

/// Provides the JSON representation of predefined genesis config for given `id`.
pub fn get_preset(id: &PresetId) -> Option<Vec<u8>> {
	let patch = match id.as_ref() {
		sp_genesis_builder::DEV_RUNTIME_PRESET => development_config_genesis(),
		sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET => local_config_genesis(),
		_ => return None,
	};
	Some(
		serde_json::to_string(&patch)
			.expect("serialization to json is expected to work. qed.")
			.into_bytes(),
	)
}

/// List of supported presets.
pub fn preset_names() -> Vec<PresetId> {
	vec![
		PresetId::from(sp_genesis_builder::DEV_RUNTIME_PRESET),
		PresetId::from(sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET),
	]
}
