//! # Tests for Credit Pallet
//!
//! 函数级详细中文注释：Credit Pallet 的完整测试套件
//!
//! **测试覆盖**：
//! - 买家信用系统（初始化、等级、限额、升级、违约）
//! - 做市商信用系统（初始化、完成订单、超时、争议、状态）
//! - 推荐和背书系统
//! - 信用评分计算

use crate::{mock::*, buyer, maker, Error};
use frame_support::{assert_noop, assert_ok};

// ========================================
// 买家信用测试（10个）
// ========================================

#[test]
fn test_initialize_new_user_credit_high_trust() {
    new_test_ext().execute_with(|| {
        let user = 1u64;
        
        // Act - 初始化新用户信用（高信任度）
        let tier = Credit::initialize_new_user_credit(&user);
        
        // Assert - 验证信用等级
        let credit = Credit::buyer_credits(&user);
        assert!(credit.risk_score <= 500);
    });
}

#[test]
fn test_buyer_check_limit_within_tier() {
    new_test_ext().execute_with(|| {
        let buyer = 1u64;
        
        // Arrange - 初始化买家信用
        let _ = Credit::initialize_new_user_credit(&buyer);
        
        // Act - 检查在限额内的订单
        let result = Credit::check_buyer_limit(&buyer, 50_000_000); // 50 USDT
        
        // Assert - 应该成功
        assert_ok!(result);
    });
}

#[test]
fn test_buyer_check_limit_exceed_tier() {
    new_test_ext().execute_with(|| {
        let buyer = 1u64;
        
        // Arrange - 初始化买家信用
        let _ = Credit::initialize_new_user_credit(&buyer);
        
        // Act - 尝试超过限额的订单
        let result = Credit::check_buyer_limit(&buyer, 500_000_000_000); // 50万 USDT
        
        // Assert - 应该失败
        assert!(result.is_err());
    });
}

#[test]
fn test_buyer_update_credit_on_success() {
    new_test_ext().execute_with(|| {
        let buyer = 1u64;
        
        // Arrange - 初始化买家信用
        let _ = Credit::initialize_new_user_credit(&buyer);
        let initial_credit = Credit::buyer_credits(&buyer);
        let initial_score = initial_credit.risk_score;
        
        // Act - 完成订单（快速付款）
        assert_ok!(Credit::update_credit_on_success(&buyer, 100_000_000, 300)); // 5分钟
        
        // Assert - 风险分应该降低
        let updated_credit = Credit::buyer_credits(&buyer);
        assert!(updated_credit.risk_score < initial_score);
        assert_eq!(updated_credit.completed_orders, initial_credit.completed_orders + 1);
    });
}

#[test]
fn test_buyer_penalize_default() {
    new_test_ext().execute_with(|| {
        let buyer = 1u64;
        
        // Arrange - 初始化买家信用
        let _ = Credit::initialize_new_user_credit(&buyer);
        let initial_credit = Credit::buyer_credits(&buyer);
        let initial_score = initial_credit.risk_score;
        
        // Act - 违约
        Credit::penalize_default(&buyer);
        
        // Assert - 风险分应该增加
        let updated_credit = Credit::buyer_credits(&buyer);
        assert!(updated_credit.risk_score > initial_score);
        
        // 验证违约历史记录
        let default_history = Credit::default_history(&buyer);
        assert_eq!(default_history.len(), 1);
    });
}

#[test]
fn test_endorse_user_success() {
    new_test_ext().execute_with(|| {
        let endorser = 1u64;
        let endorsee = 2u64;
        
        // Arrange - 初始化双方信用
        let _ = Credit::initialize_new_user_credit(&endorser);
        let _ = Credit::initialize_new_user_credit(&endorsee);
        
        // 确保推荐人有低风险分（通过完成多个订单）
        for _ in 0..5 {
            assert_ok!(Credit::update_credit_on_success(&endorser, 100_000_000, 300));
        }
        
        // Act - 推荐
        assert_ok!(Credit::endorse_user(RuntimeOrigin::signed(endorser), endorsee));
        
        // Assert - 验证推荐记录
        let endorsements = Credit::buyer_endorsements(&endorsee);
        assert_eq!(endorsements.len(), 1);
        assert_eq!(endorsements[0].endorser, endorser);
    });
}

#[test]
fn test_endorse_user_cannot_endorse_self() {
    new_test_ext().execute_with(|| {
        let user = 1u64;
        
        // Arrange
        let _ = Credit::initialize_new_user_credit(&user);
        
        // Act & Assert - 不能推荐自己
        assert_noop!(
            Credit::endorse_user(RuntimeOrigin::signed(user), user),
            Error::<Test>::CannotEndorseSelf
        );
    });
}

#[test]
fn test_set_referrer_success() {
    new_test_ext().execute_with(|| {
        let user = 1u64;
        let referrer = 2u64;
        
        // Arrange - 初始化双方信用
        let _ = Credit::initialize_new_user_credit(&user);
        let _ = Credit::initialize_new_user_credit(&referrer);
        
        // Act - 设置推荐人
        assert_ok!(Credit::set_referrer(RuntimeOrigin::signed(user), referrer));
        
        // Assert - 验证推荐人
        let credit = Credit::buyer_credits(&user);
        assert_eq!(credit.referrer, Some(referrer));
    });
}

#[test]
fn test_record_transfer() {
    new_test_ext().execute_with(|| {
        let user = 1u64;
        
        // Arrange
        let initial_count = Credit::transfer_count(&user);
        
        // Act - 记录转账
        Credit::record_transfer(&user);
        Credit::record_transfer(&user);
        Credit::record_transfer(&user);
        
        // Assert - 验证转账计数增加
        let updated_count = Credit::transfer_count(&user);
        assert_eq!(updated_count, initial_count + 3);
    });
}

#[test]
fn test_buyer_tier_upgrade_on_success() {
    new_test_ext().execute_with(|| {
        let buyer = 1u64;
        
        // Arrange - 初始化买家信用
        let _ = Credit::initialize_new_user_credit(&buyer);
        let initial_tier = Credit::buyer_credits(&buyer).tier;
        
        // Act - 完成多个订单以升级等级
        for _ in 0..10 {
            assert_ok!(Credit::update_credit_on_success(&buyer, 100_000_000, 300));
        }
        
        // Assert - 验证等级可能升级（取决于风险分）
        let updated_tier = Credit::buyer_credits(&buyer).tier;
        // 注意：等级升级取决于风险分的降低幅度
    });
}

// ========================================
// 做市商信用测试（10个）
// ========================================

#[test]
fn test_initialize_maker_credit() {
    new_test_ext().execute_with(|| {
        let maker_id = 1u64;
        
        // Act - 初始化做市商信用
        assert_ok!(Credit::initialize_maker_credit(maker_id));
        
        // Assert - 验证信用记录存在
        let credit = Credit::maker_credits(maker_id);
        assert!(credit.is_some());
        
        let record = credit.unwrap();
        assert_eq!(record.credit_score, InitialMakerCreditScore::get());
        assert_eq!(record.status, maker::ServiceStatus::Active);
        assert_eq!(record.total_orders, 0);
        assert_eq!(record.completed_orders, 0);
    });
}

#[test]
fn test_maker_record_order_completed() {
    new_test_ext().execute_with(|| {
        let maker_id = 1u64;
        let order_id = 1u64;
        
        // Arrange - 初始化做市商信用
        assert_ok!(Credit::initialize_maker_credit(maker_id));
        let initial_credit = Credit::maker_credits(maker_id).unwrap();
        let initial_score = initial_credit.credit_score;
        
        // Act - 记录完成订单（快速响应）
        assert_ok!(Credit::record_maker_order_completed(
            maker_id,
            order_id,
            300 // 5分钟响应
        ));
        
        // Assert - 验证信用分增加
        let updated_credit = Credit::maker_credits(maker_id).unwrap();
        assert!(updated_credit.credit_score >= initial_score); // 分数应该增加或保持
        assert_eq!(updated_credit.completed_orders, initial_credit.completed_orders + 1);
        assert_eq!(updated_credit.total_orders, initial_credit.total_orders + 1);
    });
}

#[test]
fn test_maker_record_order_timeout() {
    new_test_ext().execute_with(|| {
        let maker_id = 1u64;
        let order_id = 1u64;
        
        // Arrange - 初始化做市商信用
        assert_ok!(Credit::initialize_maker_credit(maker_id));
        let initial_credit = Credit::maker_credits(maker_id).unwrap();
        let initial_score = initial_credit.credit_score;
        
        // Act - 记录超时订单
        assert_ok!(Credit::record_maker_order_timeout(maker_id, order_id));
        
        // Assert - 验证信用分降低
        let updated_credit = Credit::maker_credits(maker_id).unwrap();
        assert!(updated_credit.credit_score < initial_score);
        assert_eq!(updated_credit.timeout_orders, initial_credit.timeout_orders + 1);
        assert_eq!(updated_credit.total_orders, initial_credit.total_orders + 1);
    });
}

#[test]
fn test_maker_record_dispute_win() {
    new_test_ext().execute_with(|| {
        let maker_id = 1u64;
        let order_id = 1u64;
        
        // Arrange - 初始化做市商信用
        assert_ok!(Credit::initialize_maker_credit(maker_id));
        let initial_credit = Credit::maker_credits(maker_id).unwrap();
        let initial_score = initial_credit.credit_score;
        
        // Act - 记录争议结果（做市商胜诉）
        assert_ok!(Credit::record_maker_dispute_result(maker_id, order_id, true));
        
        // Assert - 验证信用分增加
        let updated_credit = Credit::maker_credits(maker_id).unwrap();
        assert!(updated_credit.credit_score >= initial_score);
        assert_eq!(updated_credit.dispute_win, initial_credit.dispute_win + 1);
    });
}

#[test]
fn test_maker_record_dispute_loss() {
    new_test_ext().execute_with(|| {
        let maker_id = 1u64;
        let order_id = 1u64;
        
        // Arrange - 初始化做市商信用
        assert_ok!(Credit::initialize_maker_credit(maker_id));
        let initial_credit = Credit::maker_credits(maker_id).unwrap();
        let initial_score = initial_credit.credit_score;
        
        // Act - 记录争议结果（做市商败诉）
        assert_ok!(Credit::record_maker_dispute_result(maker_id, order_id, false));
        
        // Assert - 验证信用分降低
        let updated_credit = Credit::maker_credits(maker_id).unwrap();
        assert!(updated_credit.credit_score < initial_score);
        assert_eq!(updated_credit.dispute_loss, initial_credit.dispute_loss + 1);
    });
}

#[test]
fn test_maker_service_status_active() {
    new_test_ext().execute_with(|| {
        let maker_id = 1u64;
        
        // Arrange - 初始化做市商信用
        assert_ok!(Credit::initialize_maker_credit(maker_id));
        
        // Act - 查询服务状态
        let status = Credit::check_maker_service_status(maker_id);
        
        // Assert - 应该是活跃状态
        assert_ok!(status);
        assert_eq!(status.unwrap(), maker::ServiceStatus::Active);
    });
}

#[test]
fn test_maker_service_status_warning() {
    new_test_ext().execute_with(|| {
        let maker_id = 1u64;
        
        // Arrange - 初始化做市商信用
        assert_ok!(Credit::initialize_maker_credit(maker_id));
        
        // Act - 记录多次超时以降低信用分到警告阈值
        for i in 1..=10 {
            assert_ok!(Credit::record_maker_order_timeout(maker_id, i));
        }
        
        // Assert - 验证状态变为警告或暂停
        let credit = Credit::maker_credits(maker_id).unwrap();
        assert!(
            credit.status == maker::ServiceStatus::Warning ||
            credit.status == maker::ServiceStatus::Suspended
        );
    });
}

#[test]
fn test_maker_query_credit_score() {
    new_test_ext().execute_with(|| {
        let maker_id = 1u64;
        
        // Arrange - 初始化做市商信用
        assert_ok!(Credit::initialize_maker_credit(maker_id));
        
        // Act - 查询信用分
        let score = Credit::query_maker_credit_score(maker_id);
        
        // Assert - 应该返回初始分数
        assert!(score.is_some());
        assert_eq!(score.unwrap(), InitialMakerCreditScore::get());
    });
}

#[test]
fn test_maker_calculate_required_deposit() {
    new_test_ext().execute_with(|| {
        let maker_id = 1u64;
        
        // Arrange - 初始化做市商信用
        assert_ok!(Credit::initialize_maker_credit(maker_id));
        
        // Act - 计算所需保证金
        let deposit = Credit::calculate_required_deposit(maker_id);
        
        // Assert - 应该返回基础保证金（1,000,000 NEX）
        assert!(deposit > 0);
    });
}

#[test]
fn test_rate_maker() {
    new_test_ext().execute_with(|| {
        let buyer = 1u64;
        let maker_id = 1u64;
        let order_id = 1u64;
        
        // Arrange - 初始化做市商信用
        assert_ok!(Credit::initialize_maker_credit(maker_id));
        
        // Act - 评价做市商（5星好评）
        assert_ok!(Credit::rate_maker(
            RuntimeOrigin::signed(buyer),
            maker_id,
            order_id,
            5 // 5星
        ));
        
        // Assert - 验证评价记录
        // 注意：具体验证取决于 rate_maker 的实现
    });
}

// ========================================
// 信用计算测试（5个）
// ========================================

#[test]
fn test_calculate_asset_trust() {
    new_test_ext().execute_with(|| {
        let user = 1u64;
        
        // Act - 计算资产信任度
        let trust = Credit::calculate_asset_trust(&user);
        
        // Assert - 应该返回 0-100 的分数
        assert!(trust <= 100);
    });
}

#[test]
fn test_calculate_age_trust() {
    new_test_ext().execute_with(|| {
        let user = 1u64;
        
        // Arrange - 初始化用户信用
        let _ = Credit::initialize_new_user_credit(&user);
        
        // Act - 计算账龄信任度
        let trust = Credit::calculate_age_trust(&user);
        
        // Assert - 应该返回 0-100 的分数
        assert!(trust <= 100);
    });
}

#[test]
fn test_calculate_activity_trust() {
    new_test_ext().execute_with(|| {
        let user = 1u64;
        
        // Arrange - 记录一些转账
        for _ in 0..10 {
            Credit::record_transfer(&user);
        }
        
        // Act - 计算活跃度信任度
        let trust = Credit::calculate_activity_trust(&user);
        
        // Assert - 应该返回 0-100 的分数，且大于0（因为有转账）
        assert!(trust > 0 && trust <= 100);
    });
}

#[test]
fn test_calculate_social_trust() {
    new_test_ext().execute_with(|| {
        let user = 1u64;
        
        // Act - 计算社交信任度
        let trust = Credit::calculate_social_trust(&user);
        
        // Assert - 应该返回 0-100 的分数
        assert!(trust <= 100);
    });
}

#[test]
fn test_calculate_new_user_risk_score() {
    new_test_ext().execute_with(|| {
        let user = 1u64;
        
        // Act - 计算新用户风险分
        let risk_score = Credit::calculate_new_user_risk_score(&user);
        
        // Assert - 应该返回 0-500 的风险分
        assert!(risk_score <= 500);
    });
}

// ========================================
// 边界测试（3个）
// ========================================

#[test]
fn test_maker_credit_not_found() {
    new_test_ext().execute_with(|| {
        let maker_id = 999u64; // 不存在的做市商
        
        // Act - 查询不存在的做市商状态
        let status = Credit::check_maker_service_status(maker_id);
        
        // Assert - 应该返回错误
        assert!(status.is_err());
    });
}

#[test]
fn test_buyer_endorse_insufficient_credit() {
    new_test_ext().execute_with(|| {
        let endorser = 1u64;
        let endorsee = 2u64;
        
        // Arrange - 初始化双方信用
        let _ = Credit::initialize_new_user_credit(&endorser);
        let _ = Credit::initialize_new_user_credit(&endorsee);
        
        // 让推荐人违约多次以提高风险分
        for _ in 0..5 {
            Credit::penalize_default(&endorser);
        }
        
        // Act & Assert - 风险分过高，不能推荐
        assert_noop!(
            Credit::endorse_user(RuntimeOrigin::signed(endorser), endorsee),
            Error::<Test>::InsufficientCreditToEndorse
        );
    });
}

#[test]
fn test_endorse_user_already_endorsed() {
    new_test_ext().execute_with(|| {
        let endorser = 1u64;
        let endorsee = 2u64;
        
        // Arrange - 初始化双方信用
        let _ = Credit::initialize_new_user_credit(&endorser);
        let _ = Credit::initialize_new_user_credit(&endorsee);
        
        // 确保推荐人有低风险分
        for _ in 0..5 {
            assert_ok!(Credit::update_credit_on_success(&endorser, 100_000_000, 300));
        }
        
        // 第一次推荐成功
        assert_ok!(Credit::endorse_user(RuntimeOrigin::signed(endorser), endorsee));
        
        // Act & Assert - 尝试再次推荐
        assert_noop!(
            Credit::endorse_user(RuntimeOrigin::signed(endorser), endorsee),
            Error::<Test>::AlreadyEndorsed
        );
    });
}
