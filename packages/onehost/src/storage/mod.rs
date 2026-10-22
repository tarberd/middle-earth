pub mod mock;
pub mod qemu_img;
pub mod traits;

pub use mock::{
    MockImageRecord, MockStorageManager, MockStorageManagerState, RecordedStorageAction,
};
pub use qemu_img::{QemuImgStorage, parse_qemu_img_info_json};
pub use traits::{
    ImageInspectionInfo, StorageError, StorageManager, execute_compute_content_hash,
    execute_cross_device_streaming_move, execute_safe_file_move,
};
