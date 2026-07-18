// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "XcodeApp",
    targets: [
        .target(name: "XcodeApp"),
        .testTarget(name: "XcodeAppTests", dependencies: ["XcodeApp"]),
    ]
)
