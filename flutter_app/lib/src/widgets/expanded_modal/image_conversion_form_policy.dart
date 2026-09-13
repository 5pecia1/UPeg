/// Presentation-only rules for image conversion; execution stays in Rust.
library;

import 'package:upeg/src/rust/api/tools.dart';
import 'package:flutter/widgets.dart' show Key;
import 'package:upeg/src/widgets/expanded_modal/form_value.dart';

abstract final class ImageConversionFields {
  static const singleTool = 'media.image_convert';
  static const batchTool = 'media.images_convert';
  static const format = 'output_format';
  static const quality = 'jpeg_quality';
  static const background = 'background';
  static const svgWidth = 'svg_width';
  static const outputLimit = 'max_output_bytes';
}

bool isImageConversionTool(String id) =>
    id == ImageConversionFields.singleTool ||
    id == ImageConversionFields.batchTool;

bool imageConversionFieldVisible(
  String toolId,
  String key,
  FormValue? Function(String) value,
) {
  if (!isImageConversionTool(toolId)) return true;
  if (key == ImageConversionFields.quality ||
      key == ImageConversionFields.background) {
    return value(ImageConversionFields.format) == const OptionValue('jpeg');
  }
  if (key == ImageConversionFields.svgWidth) {
    final input = value(
      toolId == ImageConversionFields.singleTool ? 'input' : 'images',
    );
    return input is FileFormValue && _containsSvg(input.value);
  }
  return true;
}

bool _containsSvg(CanonicalFileValue file) => switch (file.content) {
  CanonicalFileContent_Bytes() => file.name.toLowerCase().endsWith('.svg'),
  CanonicalFileContent_Directory(:final entries) => entries.any(_containsSvg),
};

String? imageConversionLabelKey(String field) => switch (field) {
  'input' => 'media.convert.input',
  'images' => 'media.convert.images',
  ImageConversionFields.format => 'media.convert.format',
  ImageConversionFields.quality => 'media.convert.quality',
  ImageConversionFields.background => 'media.convert.background',
  ImageConversionFields.svgWidth => 'media.convert.svg_width',
  ImageConversionFields.outputLimit => 'media.convert.limit',
  _ => null,
};

abstract final class ImageConversionKeys {
  static const format = Key('field-output_format');
  static const quality = Key('field-jpeg_quality');
  static const background = Key('field-background');
  static const svgWidth = Key('field-svg_width');
}
