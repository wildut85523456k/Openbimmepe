use openbim_automation::{*,worker::PythonWorker};
use openbim_document::Document;
use std::{sync::atomic::AtomicBool,time::Duration};
#[test]
fn python_requires_host_enable_and_cannot_claim_element_types() {
    let mut g=Graph { groups:vec![], version:1,nodes:vec![
        Node { id:1,kind:NodeKind::Length { metres:2. },position:[0.,0.] },
        Node { id:2,kind:NodeKind::Python { expression:"x+1".into(),quantity:Type::Length },position:[0.,0.] },
    ],wires:vec![Wire { output:0, from:1,to:2,input:0, route: None }] };
    assert!(matches!(g.preview(&Document::new()),Err(Error::PythonDisabled(2))));
    assert!(matches!(g.preview_with(&Document::new(),None,&AtomicBool::new(true)),Err(Error::Cancelled)));
    g.nodes[1].kind=NodeKind::Python { expression:"x".into(),quantity:Type::Elements };
    assert_eq!(g.validate(),Err(Error::InvalidPythonType(2)));
}
#[test]
fn invalid_inputs_fail_before_process_spawn() {
    let worker=PythonWorker { executable:"nonexistent-python".into(),timeout:Duration::from_secs(1) };
    assert!(worker.evaluate("x",&[f64::NAN],&AtomicBool::new(false)).is_err());
    assert!(worker.evaluate("x",&[1.],&AtomicBool::new(true)).is_err());
    assert!(worker.evaluate("x",&[1.],&AtomicBool::new(false)).unwrap_err().contains("Cannot start Python"));
}
// Explicit interpreter path makes subprocess tests independent of PATH aliases.
#[test]
#[ignore = "Set OPENBIM_TEST_PYTHON to Python 3, then run with --ignored"]
fn python_process_expression_and_failure() {
    let executable=std::env::var_os("OPENBIM_TEST_PYTHON").expect("Set OPENBIM_TEST_PYTHON").into();
    let worker=PythonWorker { executable,timeout:Duration::from_secs(3) };
    assert_eq!(worker.evaluate("x + 0.3",&[2.7,3.7],&AtomicBool::new(false)).unwrap(),vec![3.,4.]);
    assert!(worker.evaluate("1 / 0",&[1.],&AtomicBool::new(false)).is_err());
    assert!(worker.evaluate("__import__('os')",&[1.],&AtomicBool::new(false)).is_err());
    let worker=PythonWorker { timeout:Duration::ZERO,..worker };
    assert!(worker.evaluate("x",&[1.],&AtomicBool::new(false)).is_err());
}
