use numbers_core::{histogram, HistogramConfig, NumberRange};

#[test]
fn deserialization_validates_range_bounds_and_preserves_valid_wire_shape() {
    assert!(serde_json::from_str::<NumberRange>(r#"{"min":2,"max":1}"#).is_err());
    for range in [
        NumberRange::new(-2.0, 3.0).unwrap(),
        NumberRange::new(1.0, 1.0).unwrap(),
    ] {
        let value = serde_json::json!({"min": range.min, "max": range.max});
        assert_eq!(serde_json::to_value(range).unwrap(), value);
        assert_eq!(serde_json::from_value::<NumberRange>(value).unwrap(), range);
    }
}

#[test]
fn public_range_operations_reject_invalid_public_fields_without_panicking() {
    for range in [
        NumberRange { min: 2.0, max: 1.0 },
        NumberRange {
            min: f64::NAN,
            max: 1.0,
        },
        NumberRange {
            min: 0.0,
            max: f64::INFINITY,
        },
        NumberRange {
            min: f64::NEG_INFINITY,
            max: 0.0,
        },
        NumberRange {
            min: 0.0,
            max: f64::NAN,
        },
    ] {
        assert!(range.clamp(0.5).is_err());
        assert!(range.normalize(0.5).is_err());
        assert!(range.denormalize(0.5).is_err());
        assert!(histogram(&[0.5], HistogramConfig::new(2).unwrap().with_range(range)).is_err());
    }
}

#[test]
fn valid_range_operations_keep_clamping_and_normalization_behavior() {
    let range = NumberRange::new(-2.0, 2.0).unwrap();
    assert_eq!(range.clamp(3.0).unwrap(), 2.0);
    assert_eq!(range.normalize(0.0).unwrap(), 0.5);
    assert_eq!(range.denormalize(0.5).unwrap(), 0.0);
    let point = NumberRange::new(2.0, 2.0).unwrap();
    assert_eq!(point.normalize(3.0).unwrap(), 0.0);
    assert_eq!(point.denormalize(0.5).unwrap(), 2.0);
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(range.clamp(value).is_err());
        assert!(range.normalize(value).is_err());
        assert!(range.denormalize(value).is_err());
    }
}
