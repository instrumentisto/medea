use super::*;

impl ForeignClass for MediaStreamSettings {
    type PointedType = MediaStreamSettings;

    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_MEDIASTREAMSETTINGS }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_MEDIASTREAMSETTINGS_NATIVEPTR_FIELD }
    }

    fn box_object(this: Self) -> jlong {
        let this: Box<MediaStreamSettings> = Box::new(this);
        let this: *mut MediaStreamSettings = Box::into_raw(this);
        this as jlong
    }

    fn unbox_object(x: jlong) -> Self {
        let x: *mut MediaStreamSettings = unsafe {
            jlong_to_pointer::<MediaStreamSettings>(x).as_mut().unwrap()
        };
        let x: Box<MediaStreamSettings> = unsafe { Box::from_raw(x) };
        let x: MediaStreamSettings = *x;
        x
    }

    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType> {
        let x: *mut MediaStreamSettings = unsafe {
            jlong_to_pointer::<MediaStreamSettings>(x).as_mut().unwrap()
        };
        ::std::ptr::NonNull::<Self::PointedType>::new(x).unwrap()
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
    let ret: () = MediaStreamSettings::audio(this, constraints);
    ret
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
    let ret: () = MediaStreamSettings::device_video(this, constraints);
    ret
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
    let ret: () = MediaStreamSettings::display_video(this, constraints);
    ret
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_MediaStreamSettings_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: *mut MediaStreamSettings = unsafe {
        jlong_to_pointer::<MediaStreamSettings>(this)
            .as_mut()
            .unwrap()
    };
    let this: Box<MediaStreamSettings> = unsafe { Box::from_raw(this) };
    drop(this);
}
