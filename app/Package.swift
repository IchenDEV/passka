// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "PasskaApp",
    platforms: [.macOS(.v13)],
    products: [
        .executable(name: "PasskaApp", targets: ["PasskaApp"]),
    ],
    targets: [
        .target(
            name: "PasskaBridge",
            path: "Sources/PasskaBridge"
        ),
        .executableTarget(
            name: "PasskaApp",
            dependencies: ["PasskaBridge"],
            path: "Sources/PasskaApp"
        ),
    ]
)
