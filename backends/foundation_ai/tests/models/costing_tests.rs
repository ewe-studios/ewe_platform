use foundation_ai::costing::*;
use foundation_ai::types::base_types::{
    CostStatus, ImageContent, MessageRole, Messages, MimeType, ModelId, ModelOutput,
    ModelProviders, ModelUsageCosting, StopReason, TextContent, UsageCosting, UsageReport,
    UserModelContent,
};

#[test]
fn test_calculate_cost_basic() {
    let pricing = ModelUsageCosting {
        input: 3.0,
        output: 15.0,
        cache_read: 0.3,
        cache_write: 3.75,
    };
    let usage = UsageReport {
        input: 1000.0,
        output: 500.0,
        cache_read: 800.0,
        cache_write: 1000.0,
        total_tokens: 1500.0,
        cost: UsageCosting::zero(CostStatus::Actual),
    };

    let result = calculate_cost(&pricing, &usage, CostStatus::Actual);
    assert!((result.input - 0.003).abs() < 1e-9);
    assert!((result.output - 0.0075).abs() < 1e-9);
    assert!((result.cache_read - 0.00024).abs() < 1e-9);
    assert!((result.cache_write - 0.00375).abs() < 1e-9);
    assert_eq!(result.status, CostStatus::Actual);
}

#[test]
fn test_calculate_cost_free_model() {
    let pricing = ModelUsageCosting {
        input: 0.0,
        output: 0.0,
        cache_read: 0.0,
        cache_write: 0.0,
    };
    let usage = UsageReport {
        input: 1000.0,
        output: 500.0,
        cache_read: 0.0,
        cache_write: 0.0,
        total_tokens: 1500.0,
        cost: UsageCosting::zero(CostStatus::Actual),
    };

    let result = calculate_cost(&pricing, &usage, CostStatus::Actual);
    assert_eq!(result.total(), 0.0);
    assert_eq!(result.status, CostStatus::Actual);
}

#[test]
fn test_usage_costing_zero() {
    let c = UsageCosting::zero(CostStatus::Estimated);
    assert_eq!(c.total(), 0.0);
    assert_eq!(c.status, CostStatus::Estimated);
}

#[test]
fn test_cost_accumulator() {
    let mut acc = CostAccumulator::new();
    assert_eq!(acc.call_count(), 0);

    let c1 = UsageCosting {
        currency: String::from("USD"),
        input: 0.003,
        output: 0.0075,
        cache_read: 0.0,
        cache_write: 0.00375,
        total_tokens: 1000.0,
        status: CostStatus::Actual,
    };
    acc.add(&c1);
    assert_eq!(acc.call_count(), 1);

    let c2 = UsageCosting {
        currency: String::from("USD"),
        input: 0.006,
        output: 0.015,
        cache_read: 0.001,
        cache_write: 0.0075,
        total_tokens: 2000.0,
        status: CostStatus::Actual,
    };
    acc.add(&c2);
    assert_eq!(acc.call_count(), 2);

    let result = acc.result();
    assert!((result.input - 0.009).abs() < 1e-9);
    assert!((result.output - 0.0225).abs() < 1e-9);
    assert!((result.cache_read - 0.001).abs() < 1e-9);
    assert!((result.cache_write - 0.01125).abs() < 1e-9);
    assert!((result.total() - 0.04375).abs() < 1e-9);
    assert_eq!(result.status, CostStatus::Actual);
}

#[test]
fn test_cost_accumulator_empty() {
    let acc = CostAccumulator::new();
    let result = acc.result();
    assert_eq!(result.total(), 0.0);
    assert_eq!(result.status, CostStatus::Unknown);
}

#[test]
fn test_estimate_tokens_text_only() {
    use foundation_ai::types::base_types::{
        ModelId, ModelProviders, StopReason, TextContent, UserModelContent,
    };

    let messages = vec![
        Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: MessageRole::User,
            content: UserModelContent::Text(TextContent {
                content: "Hello, world!".to_string(),
                signature: None,
            }),
            signature: None,
        },
        Messages::Assistant {
            id: foundation_compact::ids::new_scru128(),
            model: ModelId::Name("test".to_string(), None),
            timestamp: foundation_compact::SystemTime::now(),
            usage: UsageReport {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
                total_tokens: 0.0,
                cost: UsageCosting::zero(CostStatus::Actual),
            },
            content: ModelOutput::Text(TextContent {
                content: "Hi there!".to_string(),
                signature: None,
            }),
            stop_reason: StopReason::Stop,
            provider: ModelProviders::ANTHROPIC,
            error_detail: None,
            signature: None,
            metadata: None,
        },
    ];

    let report = estimate_tokens(&messages);
    // 2 messages * 4 overhead = 8
    // "Hello, world!" = 13 chars / 4 = 3.25
    // "Hi there!" = 9 chars / 4 = 2.25
    // input = 8 + 3.25 + 2.25 = 13.5
    assert!((report.input - 13.5).abs() < 0.01);
    assert_eq!(report.output, 0.0);
}

#[test]
fn test_estimate_tokens_with_image() {
    let messages = vec![Messages::User {
        id: foundation_compact::ids::new_scru128(),
        role: MessageRole::User,
        content: UserModelContent::Image(ImageContent {
            b64: "base64data".to_string(),
            mime_type: MimeType::ImagePng,
        }),
        signature: None,
    }];

    let report = estimate_tokens(&messages);
    // 1 message * 4 overhead = 4
    // base64data = 10 chars / 20 = 0.5
    // input = 4.5
    // images = 1000
    assert!((report.input - 4.5).abs() < 0.01);
    assert!((report.total_tokens - 1004.5).abs() < 0.01);
}

#[test]
fn test_message_cost() {
    let messages = vec![Messages::Assistant {
        id: foundation_compact::ids::new_scru128(),
        model: ModelId::Name("test".to_string(), None),
        timestamp: foundation_compact::SystemTime::now(),
        usage: UsageReport {
            input: 100.0,
            output: 50.0,
            cache_read: 80.0,
            cache_write: 100.0,
            total_tokens: 150.0,
            cost: UsageCosting {
                currency: String::from("USD"),
                input: 0.003,
                output: 0.0075,
                cache_read: 0.0,
                cache_write: 0.00375,
                total_tokens: 150.0,
                status: CostStatus::Actual,
            },
        },
        content: ModelOutput::Text(TextContent {
            content: "Hi".to_string(),
            signature: None,
        }),
        stop_reason: StopReason::Stop,
        provider: ModelProviders::ANTHROPIC,
        error_detail: None,
        signature: None,
        metadata: None,
    }];

    let cost = message_cost(&messages);
    assert_eq!(cost.input, 0.003);
    assert_eq!(cost.output, 0.0075);
    assert_eq!(cost.cache_read, 0.0);
    assert_eq!(cost.cache_write, 0.00375);
    assert_eq!(cost.status, CostStatus::Actual);
}
