use sc_service::ChainType;
use sc_telemetry::serde_json::json;
use nexus_runtime::WASM_BINARY;

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec;

fn chain_properties() -> sc_service::Properties {
	json!({
		"tokenSymbol": "NEX",
		"tokenDecimals": 12,
		"ss58Format": 273,
		"genesisInscription": concat!(
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
		)
	})
	.as_object()
	.cloned()
	.unwrap()
}

pub fn development_chain_spec() -> Result<ChainSpec, String> {
	Ok(ChainSpec::builder(
		WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?,
		None,
	)
	.with_name("Nexus Development")
	.with_id("nexus_dev")
	.with_chain_type(ChainType::Development)
	.with_genesis_config_preset_name(sp_genesis_builder::DEV_RUNTIME_PRESET)
	.with_properties(chain_properties())
	.build())
}

pub fn local_chain_spec() -> Result<ChainSpec, String> {
	Ok(ChainSpec::builder(
		WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?,
		None,
	)
	.with_name("Nexus Local Testnet")
	.with_id("nexus_local")
	.with_chain_type(ChainType::Local)
	.with_genesis_config_preset_name(sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET)
	.with_properties(chain_properties())
	.build())
}
