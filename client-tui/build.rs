fn main() {
    let proto_folder = build_common::get_proto_folder();
    build_common::compile_protos_folder(&proto_folder).expect("Unable to compile protos");
}