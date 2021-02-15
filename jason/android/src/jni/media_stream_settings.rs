use super::*;

impl ForeignClass for MediaStreamSettings {
    type PointedType = MediaStreamSettings;

    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_MEDIASTREAMSETTINGS }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_MEDIASTREAMSETTINGS_NATIVEPTR_FIELD }
    }

    fn box_object(self) -> jlong {
        Box::into_raw(Box::new(self)) as i64
    }

    fn get_ptr(x: jlong) -> ptr::NonNull<Self::PointedType> {
        let x: *mut MediaStreamSettings = unsafe {
            jlong_to_pointer::<MediaStreamSettings>(x).as_mut().unwrap()
        };
        ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_MediaStreamSettings_nativeAudio(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    constraints: jlong,
) {
    let constraints: *mut AudioTrackConstraints = unsafe {
        jlong_to_pointer::<AudioTrackConstraints>(constraints)
            .as_mut()
            .unwrap()
    };
    let constraints: Box<AudioTrackConstraints> =
        unsafe { Box::from_raw(constraints) };
    let constraints: AudioTrackConstraints = *constraints;
    let this: &mut MediaStreamSettings = unsafe {
        jlong_to_pointer::<MediaStreamSettings>(this)
            .as_mut()
            .unwrap()
    };
    this.audio(constraints)
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_MediaStreamSettings_nativeDeviceVideo(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    constraints: jlong,
) {
    let constraints: *mut DeviceVideoTrackConstraints = unsafe {
        jlong_to_pointer::<DeviceVideoTrackConstraints>(constraints)
            .as_mut()
            .unwrap()
    };
    let constraints: Box<DeviceVideoTrackConstraints> =
        unsafe { Box::from_raw(constraints) };
    let constraints: DeviceVideoTrackConstraints = *constraints;
    let this: &mut MediaStreamSettings = unsafe {
        jlong_to_pointer::<MediaStreamSettings>(this)
            .as_mut()
            .unwrap()
    };
    this.device_video(constraints)
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_MediaStreamSettings_nativeDisplayVideo(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    constraints: jlong,
) {
    let constraints: *mut DisplayVideoTrackConstraints = unsafe {
        jlong_to_pointer::<DisplayVideoTrackConstraints>(constraints)
            .as_mut()
            .unwrap()
    };
    let constraints: Box<DisplayVideoTrackConstraints> =
        unsafe { Box::from_raw(constraints) };
    let constraints: DisplayVideoTrackConstraints = *constraints;
    let this: &mut MediaStreamSettings = unsafe {
        jlong_to_pointer::<MediaStreamSettings>(this)
            .as_mut()
            .unwrap()
    };
    this.display_video(constraints);
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_MediaStreamSettings_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    MediaStreamSettings::get_boxed(ptr);
}
