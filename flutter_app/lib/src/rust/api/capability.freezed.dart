// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'capability.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// dart format off
T _$identity<T>(T value) => value;
/// @nodoc
mixin _$DispatchCapabilityDto {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is DispatchCapabilityDto);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'DispatchCapabilityDto()';
}


}

/// @nodoc
class $DispatchCapabilityDtoCopyWith<$Res>  {
$DispatchCapabilityDtoCopyWith(DispatchCapabilityDto _, $Res Function(DispatchCapabilityDto) __);
}


/// Adds pattern-matching-related methods to [DispatchCapabilityDto].
extension DispatchCapabilityDtoPatterns on DispatchCapabilityDto {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( DispatchCapabilityDto_Supported value)?  supported,TResult Function( DispatchCapabilityDto_Unsupported value)?  unsupported,required TResult orElse(),}){
final _that = this;
switch (_that) {
case DispatchCapabilityDto_Supported() when supported != null:
return supported(_that);case DispatchCapabilityDto_Unsupported() when unsupported != null:
return unsupported(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( DispatchCapabilityDto_Supported value)  supported,required TResult Function( DispatchCapabilityDto_Unsupported value)  unsupported,}){
final _that = this;
switch (_that) {
case DispatchCapabilityDto_Supported():
return supported(_that);case DispatchCapabilityDto_Unsupported():
return unsupported(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( DispatchCapabilityDto_Supported value)?  supported,TResult? Function( DispatchCapabilityDto_Unsupported value)?  unsupported,}){
final _that = this;
switch (_that) {
case DispatchCapabilityDto_Supported() when supported != null:
return supported(_that);case DispatchCapabilityDto_Unsupported() when unsupported != null:
return unsupported(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function()?  supported,TResult Function( UnsupportedReasonDto reason)?  unsupported,required TResult orElse(),}) {final _that = this;
switch (_that) {
case DispatchCapabilityDto_Supported() when supported != null:
return supported();case DispatchCapabilityDto_Unsupported() when unsupported != null:
return unsupported(_that.reason);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function()  supported,required TResult Function( UnsupportedReasonDto reason)  unsupported,}) {final _that = this;
switch (_that) {
case DispatchCapabilityDto_Supported():
return supported();case DispatchCapabilityDto_Unsupported():
return unsupported(_that.reason);}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function()?  supported,TResult? Function( UnsupportedReasonDto reason)?  unsupported,}) {final _that = this;
switch (_that) {
case DispatchCapabilityDto_Supported() when supported != null:
return supported();case DispatchCapabilityDto_Unsupported() when unsupported != null:
return unsupported(_that.reason);case _:
  return null;

}
}

}

/// @nodoc


class DispatchCapabilityDto_Supported extends DispatchCapabilityDto {
  const DispatchCapabilityDto_Supported(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is DispatchCapabilityDto_Supported);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'DispatchCapabilityDto.supported()';
}


}




/// @nodoc


class DispatchCapabilityDto_Unsupported extends DispatchCapabilityDto {
  const DispatchCapabilityDto_Unsupported({required this.reason}): super._();


 final  UnsupportedReasonDto reason;

/// Create a copy of DispatchCapabilityDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$DispatchCapabilityDto_UnsupportedCopyWith<DispatchCapabilityDto_Unsupported> get copyWith => _$DispatchCapabilityDto_UnsupportedCopyWithImpl<DispatchCapabilityDto_Unsupported>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is DispatchCapabilityDto_Unsupported&&(identical(other.reason, reason) || other.reason == reason));
}


@override
int get hashCode => Object.hash(runtimeType,reason);

@override
String toString() {
  return 'DispatchCapabilityDto.unsupported(reason: $reason)';
}


}

/// @nodoc
abstract mixin class $DispatchCapabilityDto_UnsupportedCopyWith<$Res> implements $DispatchCapabilityDtoCopyWith<$Res> {
  factory $DispatchCapabilityDto_UnsupportedCopyWith(DispatchCapabilityDto_Unsupported value, $Res Function(DispatchCapabilityDto_Unsupported) _then) = _$DispatchCapabilityDto_UnsupportedCopyWithImpl;
@useResult
$Res call({
 UnsupportedReasonDto reason
});




}
/// @nodoc
class _$DispatchCapabilityDto_UnsupportedCopyWithImpl<$Res>
    implements $DispatchCapabilityDto_UnsupportedCopyWith<$Res> {
  _$DispatchCapabilityDto_UnsupportedCopyWithImpl(this._self, this._then);

  final DispatchCapabilityDto_Unsupported _self;
  final $Res Function(DispatchCapabilityDto_Unsupported) _then;

/// Create a copy of DispatchCapabilityDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? reason = null,}) {
  return _then(DispatchCapabilityDto_Unsupported(
reason: null == reason ? _self.reason : reason // ignore: cast_nullable_to_non_nullable
as UnsupportedReasonDto,
  ));
}


}

// dart format on
