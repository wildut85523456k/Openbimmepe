//! The application's own lines (`U|kind=<kind>|key=value...`): the open place for what the app keeps that IFC has no
//! place for. A new kind is added without touching the format.

use openbim_document::{native, Document};

#[test]
fn a_line_is_kept_by_its_kind_and_replaced_by_the_next_of_that_kind() {
    let mut d = Document::new();
    d.put_app_record("cost-codes", Some("U|kind=cost-codes|scheme=CSI|ver=2".into())).unwrap();
    d.put_app_record("phasing", Some("U|kind=phasing|n=3".into())).unwrap();
    assert_eq!(d.app_record("cost-codes"), Some("U|kind=cost-codes|scheme=CSI|ver=2"));
    d.put_app_record("cost-codes", Some("U|kind=cost-codes|scheme=Uniclass|ver=1".into())).unwrap();
    assert_eq!(d.app_records().len(), 2, "one line for each kind");
    assert_eq!(d.app_record("cost-codes"), Some("U|kind=cost-codes|scheme=Uniclass|ver=1"));
    d.put_app_record("phasing", None).unwrap();
    assert_eq!(d.app_record("phasing"), None);
    // a kind is not a prefix of another
    d.put_app_record("cost", Some("U|kind=cost|x=1".into())).unwrap();
    assert_eq!(d.app_record("cost"), Some("U|kind=cost|x=1"));
    assert_eq!(d.app_record("cost-codes"), Some("U|kind=cost-codes|scheme=Uniclass|ver=1"));
}

#[test]
fn a_line_that_is_not_one_line_of_its_kind_is_refused() {
    let mut d = Document::new();
    assert!(d.put_app_record("", Some("U|kind=|x=1".into())).is_err());
    assert!(d.put_app_record("Bad Kind", Some("U|kind=Bad Kind|x=1".into())).is_err());
    assert!(d.put_app_record("a", Some("U|kind=b|x=1".into())).is_err(), "the line says another kind");
    assert!(d.put_app_record("a", Some("E|kind=a".into())).is_err(), "not a U line");
    assert!(d.put_app_record("a", Some("U|kind=a|x=1\nU|kind=a|y=2".into())).is_err(), "two lines");
    assert!(d.app_records().is_empty());
    assert_eq!(native::app_record_kind("U|kind=georef|crs=x"), Some("georef"));
    assert_eq!(native::app_record_kind("U|kind=solo"), Some("solo"));
    assert_eq!(native::app_record_kind("U|crs=x"), None);
}

#[test]
fn every_kind_is_saved_with_the_model_and_read_back() {
    let mut d = Document::new();
    d.place_level("Level 1", 0.0).unwrap();
    d.put_app_record("cost-codes", Some("U|kind=cost-codes|scheme=CSI~00007cx".into())).unwrap();
    d.put_app_record("georef", Some("U|kind=georef|crs=EPSG~00003a32648".into())).unwrap();
    let lines = native::records(&d);
    assert_eq!(native::parse_app_records(&lines), d.app_records());
    assert!(lines.iter().filter(|l| l.starts_with("U|")).count() == 2);
}
