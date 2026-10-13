//! Real socket response boundaries, including allocations after handler construction.

#![cfg(unix)]

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::{Cell, RefCell},
    fs,
    io::{Read, Write},
    os::unix::{fs::PermissionsExt, net::UnixStream},
    thread,
    time::Duration,
};

use ghostrace::{
    LocalService, ServiceCapability, ServiceError, ServiceHandler, ServiceRequest, ServiceResponse,
    LOCAL_SERVICE_PROTOCOL_VERSION, MAX_SERVICE_MESSAGE_BYTES,
};
use serde_json::Value;
use uuid::Uuid;

thread_local! {
    static MEASURING: Cell<bool> = const { Cell::new(false) };
    static LARGEST_ALLOCATION: Cell<usize> = const { Cell::new(0) };
}

struct MeasuredAllocator;

fn record_allocation(bytes: usize) {
    let _ = MEASURING.try_with(|measuring| {
        if measuring.get() {
            let _ = LARGEST_ALLOCATION.try_with(|largest| largest.set(largest.get().max(bytes)));
        }
    });
}

// SAFETY: Every allocation operation is forwarded unchanged to the system
// allocator. The thread-local counters neither allocate nor change pointers.
unsafe impl GlobalAlloc for MeasuredAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation(layout.size());
        // SAFETY: The caller's layout is forwarded to the system allocator.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_allocation(layout.size());
        // SAFETY: The caller's layout is forwarded to the system allocator.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: The caller's pointer and layout are forwarded unchanged.
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record_allocation(size);
        // SAFETY: The caller's pointer, layout and size are forwarded unchanged.
        unsafe { System.realloc(pointer, layout, size) }
    }
}

#[global_allocator]
static ALLOCATOR: MeasuredAllocator = MeasuredAllocator;

struct PreparedResult(RefCell<Option<Value>>);

impl ServiceHandler for PreparedResult {
    fn handle(&self, _: &ServiceRequest) -> Result<Value, ServiceError> {
        let result = self.0.borrow_mut().take().expect("one prepared result");
        // Count only the server thread's allocations after the result exists.
        // Moving this Value into the service requires no copy of its string.
        LARGEST_ALLOCATION.with(|largest| largest.set(0));
        MEASURING.with(|measuring| measuring.set(true));
        Ok(result)
    }
}

fn response_bytes(result: Value) -> (Vec<u8>, usize, Uuid) {
    let parent = tempfile::tempdir().expect("private parent");
    fs::set_permissions(parent.path(), fs::Permissions::from_mode(0o700)).expect("private mode");
    let mut service = LocalService::bind(&parent.path().join("service"), [ServiceCapability::Read])
        .expect("bind");
    let request_id = Uuid::from_u128(1);
    let request = ServiceRequest {
        protocol_version: LOCAL_SERVICE_PROTOCOL_VERSION,
        service_instance: service.instance(),
        request_id,
        deadline_ms: 5_000,
        capability: ServiceCapability::Read,
        method: "status".to_owned(),
        params: Value::Null,
    };
    let path = service.socket_path().to_path_buf();
    let request_body = serde_json::to_vec(&request).expect("request JSON");
    let client = thread::spawn(move || {
        let mut stream = UnixStream::connect(path).expect("connect");
        stream.set_read_timeout(Some(Duration::from_secs(10))).expect("client timeout");
        stream.write_all(&(request_body.len() as u32).to_be_bytes()).expect("request length");
        stream.write_all(&request_body).expect("request body");
        let mut prefix = [0; 4];
        stream.read_exact(&mut prefix).expect("response length");
        let length = u32::from_be_bytes(prefix) as usize;
        assert!(length <= MAX_SERVICE_MESSAGE_BYTES, "oversized response reached the socket");
        let mut body = vec![0; length];
        stream.read_exact(&mut body).expect("response body");
        body
    });
    let handler = PreparedResult(RefCell::new(Some(result)));
    let served = service.serve_one(&handler);
    MEASURING.with(|measuring| measuring.set(false));
    let largest = LARGEST_ALLOCATION.with(Cell::get);
    served.expect("serve prepared result");
    (client.join().expect("client"), largest, request_id)
}

#[test]
fn oversized_result_serialization_never_allocates_past_the_response_bound() {
    let sentinel = "synthetic-response-secret";
    let result = Value::String(sentinel.repeat(100_000));
    let (body, largest, _) = response_bytes(result);
    assert_eq!(body, br#"{"status":"refused","error":"too_large"}"#);
    assert!(!String::from_utf8_lossy(&body).contains(sentinel));
    println!("largest post-handler allocation: {largest} bytes");
    assert!(
        largest <= MAX_SERVICE_MESSAGE_BYTES,
        "serialization allocated {largest} bytes for a {MAX_SERVICE_MESSAGE_BYTES}-byte response bound"
    );
}

#[test]
fn response_byte_boundary_preserves_the_existing_json_wire_format() {
    // The fixed UUID and empty string response occupy 79 bytes on the v1 wire.
    // Payload lengths are derived independently of the service encoder.
    const ENVELOPE_BYTES: usize = 79;
    for (extra, admitted) in [(-1_i32, true), (0, true), (1, false)] {
        let payload_bytes = (MAX_SERVICE_MESSAGE_BYTES - ENVELOPE_BYTES) as i32 + extra;
        let text = "a".repeat(payload_bytes as usize);
        let (body, largest, request_id) = response_bytes(Value::String(text.clone()));
        assert!(largest <= MAX_SERVICE_MESSAGE_BYTES);
        if admitted {
            let expected = format!(
                "{{\"status\":\"ok\",\"request_id\":\"{request_id}\",\"result\":\"{text}\"}}"
            );
            assert_eq!(body, expected.as_bytes());
            assert_eq!(body.len(), (MAX_SERVICE_MESSAGE_BYTES as i32 + extra) as usize);
        } else {
            assert_eq!(body, br#"{"status":"refused","error":"too_large"}"#);
        }
    }
}

#[test]
fn escaped_and_utf8_results_are_bounded_by_encoded_bytes() {
    for (text, admitted) in [
        ("é🦀\n\"\\".to_owned(), true),
        ("\u{0000}".repeat(11_000), false),
        ("🦀".repeat(17_000), false),
    ] {
        let (body, largest, request_id) = response_bytes(Value::String(text.clone()));
        assert!(largest <= MAX_SERVICE_MESSAGE_BYTES);
        let response: ServiceResponse = serde_json::from_slice(&body).expect("valid response JSON");
        match response {
            ServiceResponse::Ok { request_id: actual_id, result } if admitted => {
                assert_eq!(actual_id, request_id);
                assert_eq!(result, Value::String(text));
                assert_eq!(body, format!("{{\"status\":\"ok\",\"request_id\":\"{request_id}\",\"result\":\"é🦀\\n\\\"\\\\\"}}").as_bytes());
            }
            ServiceResponse::Refused { error } if !admitted => {
                assert_eq!(error, ServiceError::TooLarge);
            }
            _ => panic!("unexpected boundary outcome"),
        }
    }
}
