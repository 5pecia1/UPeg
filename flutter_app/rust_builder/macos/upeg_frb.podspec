#
# To learn more about a Podspec see http://guides.cocoapods.org/syntax/podspec.html.
# Run `pod lib lint upeg_frb.podspec` to validate before publishing.
#
Pod::Spec.new do |s|
  s.name             = 'upeg_frb'
  s.version          = '0.0.1'
  s.summary          = 'upeg Flutter Rust bridge.'
  s.description      = <<-DESC
Rust bridge used by the upeg Flutter surface.
                       DESC
  s.homepage         = 'https://github.com/5pecia1/UPeg'
  s.license          = { :type => 'Apache-2.0' }
  s.author           = { 'upeg contributors' => 'https://github.com/5pecia1/UPeg' }

  # This will ensure the source files in Classes/ are included in the native
  # builds of apps using this FFI plugin. Podspec does not support relative
  # paths, so Classes contains a forwarder C file that relatively imports
  # `../src/*` so that the C sources can be shared among all target platforms.
  s.source           = { :path => '.' }
  s.source_files     = 'Classes/**/*'
  s.dependency 'FlutterMacOS'

  s.platform = :osx, '10.11'
  s.pod_target_xcconfig = { 'DEFINES_MODULE' => 'YES' }
  s.swift_version = '5.0'

  s.script_phase = {
    :name => 'Build Rust library',
    # First argument is relative path to the `rust` folder, second is name of rust library
    :script => 'sh "$PODS_TARGET_SRCROOT/../cargokit/build_pod.sh" ../../../upeg-frb upeg_frb',
    :execution_position => :before_compile,
    :input_files => ['${BUILT_PRODUCTS_DIR}/cargokit_phony'],
    # Let XCode know that the static library referenced in -force_load below is
    # created by this build step.
    :output_files => ["${BUILT_PRODUCTS_DIR}/libupeg_frb.a"],
  }
  s.pod_target_xcconfig = {
    'DEFINES_MODULE' => 'YES',
    # Flutter.framework does not contain a i386 slice.
    'EXCLUDED_ARCHS[sdk=iphonesimulator*]' => 'i386',
    'OTHER_LDFLAGS' => '-force_load ${BUILT_PRODUCTS_DIR}/libupeg_frb.a',
  }
end
