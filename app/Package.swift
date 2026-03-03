// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "PasskaApp",
    platforms: [.macOS(.v13)],
    products: [
        .executable(name: "PasskaApp", targets: ["PasskaApp"]),
    ],
    targets: [
        .systemLibrary(
            name: "PasskaHeaders",
            path: "Sources/PasskaHeaders"
        ),
        .target(
            name: "PasskaBridge",
            dependencies: ["PasskaHeaders"],
            path: "Sources/PasskaBridge"
        ),
        .executableTarget(
            name: "PasskaApp",
            dependencies: ["PasskaBridge"],
            path: "Sources/PasskaApp",
            linkerSettings: [
                .unsafeFlags([
                    "-L../../target/release",
                    "-lpasska_ffi",
                ]),
                .linkedFramework("Security"),
                .linkedFramework("LocalAuthentication"),
            ]
        ),
    ]
)
