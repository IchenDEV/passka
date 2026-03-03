public func passka_list_credentials<GenericIntoRustString: IntoRustString>(_ type_filter: Optional<GenericIntoRustString>) -> RustString {
    RustString(ptr: __swift_bridge__$passka_list_credentials({ if let rustString = optionalStringIntoRustString(type_filter) { rustString.isOwned = false; return rustString.ptr } else { return nil } }()))
}
public func passka_get_credential_meta<GenericToRustStr: ToRustStr>(_ name: GenericToRustStr) -> Optional<RustString> {
    return name.toRustStr({ nameAsRustStr in
        { let val = __swift_bridge__$passka_get_credential_meta(nameAsRustStr); if val != nil { return RustString(ptr: val!) } else { return nil } }()
    })
}
public func passka_get_credential_value<GenericToRustStr: ToRustStr>(_ name: GenericToRustStr, _ field: GenericToRustStr) -> Optional<RustString> {
    return field.toRustStr({ fieldAsRustStr in
        return name.toRustStr({ nameAsRustStr in
        { let val = __swift_bridge__$passka_get_credential_value(nameAsRustStr, fieldAsRustStr); if val != nil { return RustString(ptr: val!) } else { return nil } }()
    })
    })
}
public func passka_add_credential<GenericToRustStr: ToRustStr>(_ name: GenericToRustStr, _ cred_type: GenericToRustStr, _ data_json: GenericToRustStr, _ description: GenericToRustStr) -> RustString {
    return description.toRustStr({ descriptionAsRustStr in
        return data_json.toRustStr({ data_jsonAsRustStr in
        return cred_type.toRustStr({ cred_typeAsRustStr in
        return name.toRustStr({ nameAsRustStr in
        RustString(ptr: __swift_bridge__$passka_add_credential(nameAsRustStr, cred_typeAsRustStr, data_jsonAsRustStr, descriptionAsRustStr))
    })
    })
    })
    })
}
public func passka_update_credential<GenericToRustStr: ToRustStr>(_ name: GenericToRustStr, _ field: GenericToRustStr, _ value: GenericToRustStr) -> RustString {
    return value.toRustStr({ valueAsRustStr in
        return field.toRustStr({ fieldAsRustStr in
        return name.toRustStr({ nameAsRustStr in
        RustString(ptr: __swift_bridge__$passka_update_credential(nameAsRustStr, fieldAsRustStr, valueAsRustStr))
    })
    })
    })
}
public func passka_remove_credential<GenericToRustStr: ToRustStr>(_ name: GenericToRustStr) -> RustString {
    return name.toRustStr({ nameAsRustStr in
        RustString(ptr: __swift_bridge__$passka_remove_credential(nameAsRustStr))
    })
}
public func passka_refresh_token<GenericToRustStr: ToRustStr>(_ name: GenericToRustStr) -> RustString {
    return name.toRustStr({ nameAsRustStr in
        RustString(ptr: __swift_bridge__$passka_refresh_token(nameAsRustStr))
    })
}


