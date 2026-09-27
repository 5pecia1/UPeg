/// Overridable seams for the pure Rust presentation resolvers.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/presentation_view.dart';

typedef PresentationRowsResolver =
    PresentationRowsDto Function({
      required String toolId,
      required String outputsJson,
    });

typedef PresentationBindingsResolver =
    ActionBindingResolutionDto Function({
      required String toolId,
      required String actionId,
      required String currentInputsJson,
      String? selectedRowJson,
      required String outputsJson,
    });

typedef PresentationViewResolver =
    PresentationViewDto Function({
      required String toolId,
      required String outputsJson,
    });

final presentationRowsResolverProvider = Provider<PresentationRowsResolver>(
  (ref) => resolveToolPresentationRows,
);

final presentationBindingsResolverProvider =
    Provider<PresentationBindingsResolver>((ref) => resolveToolActionBindings);

final presentationViewResolverProvider = Provider<PresentationViewResolver>(
  (ref) => resolveToolPresentationView,
);
