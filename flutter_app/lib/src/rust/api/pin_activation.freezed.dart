// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'pin_activation.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// dart format off
T _$identity<T>(T value) => value;
/// @nodoc
mixin _$PinActivationDto {

 String get toolId;
/// Create a copy of PinActivationDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$PinActivationDtoCopyWith<PinActivationDto> get copyWith => _$PinActivationDtoCopyWithImpl<PinActivationDto>(this as PinActivationDto, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is PinActivationDto&&(identical(other.toolId, toolId) || other.toolId == toolId));
}


@override
int get hashCode => Object.hash(runtimeType,toolId);

@override
String toString() {
  return 'PinActivationDto(toolId: $toolId)';
}


}

/// @nodoc
abstract mixin class $PinActivationDtoCopyWith<$Res>  {
  factory $PinActivationDtoCopyWith(PinActivationDto value, $Res Function(PinActivationDto) _then) = _$PinActivationDtoCopyWithImpl;
@useResult
$Res call({
 String toolId
});




}
/// @nodoc
class _$PinActivationDtoCopyWithImpl<$Res>
    implements $PinActivationDtoCopyWith<$Res> {
  _$PinActivationDtoCopyWithImpl(this._self, this._then);

  final PinActivationDto _self;
  final $Res Function(PinActivationDto) _then;

/// Create a copy of PinActivationDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') @override $Res call({Object? toolId = null,}) {
  return _then(_self.copyWith(
toolId: null == toolId ? _self.toolId : toolId // ignore: cast_nullable_to_non_nullable
as String,
  ));
}

}


/// Adds pattern-matching-related methods to [PinActivationDto].
extension PinActivationDtoPatterns on PinActivationDto {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( PinActivationDto_DispatchImmediate value)?  dispatchImmediate,TResult Function( PinActivationDto_OpenModal value)?  openModal,TResult Function( PinActivationDto_OpenEmbed value)?  openEmbed,required TResult orElse(),}){
final _that = this;
switch (_that) {
case PinActivationDto_DispatchImmediate() when dispatchImmediate != null:
return dispatchImmediate(_that);case PinActivationDto_OpenModal() when openModal != null:
return openModal(_that);case PinActivationDto_OpenEmbed() when openEmbed != null:
return openEmbed(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( PinActivationDto_DispatchImmediate value)  dispatchImmediate,required TResult Function( PinActivationDto_OpenModal value)  openModal,required TResult Function( PinActivationDto_OpenEmbed value)  openEmbed,}){
final _that = this;
switch (_that) {
case PinActivationDto_DispatchImmediate():
return dispatchImmediate(_that);case PinActivationDto_OpenModal():
return openModal(_that);case PinActivationDto_OpenEmbed():
return openEmbed(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( PinActivationDto_DispatchImmediate value)?  dispatchImmediate,TResult? Function( PinActivationDto_OpenModal value)?  openModal,TResult? Function( PinActivationDto_OpenEmbed value)?  openEmbed,}){
final _that = this;
switch (_that) {
case PinActivationDto_DispatchImmediate() when dispatchImmediate != null:
return dispatchImmediate(_that);case PinActivationDto_OpenModal() when openModal != null:
return openModal(_that);case PinActivationDto_OpenEmbed() when openEmbed != null:
return openEmbed(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String toolId)?  dispatchImmediate,TResult Function( String toolId)?  openModal,TResult Function( String toolId)?  openEmbed,required TResult orElse(),}) {final _that = this;
switch (_that) {
case PinActivationDto_DispatchImmediate() when dispatchImmediate != null:
return dispatchImmediate(_that.toolId);case PinActivationDto_OpenModal() when openModal != null:
return openModal(_that.toolId);case PinActivationDto_OpenEmbed() when openEmbed != null:
return openEmbed(_that.toolId);case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String toolId)  dispatchImmediate,required TResult Function( String toolId)  openModal,required TResult Function( String toolId)  openEmbed,}) {final _that = this;
switch (_that) {
case PinActivationDto_DispatchImmediate():
return dispatchImmediate(_that.toolId);case PinActivationDto_OpenModal():
return openModal(_that.toolId);case PinActivationDto_OpenEmbed():
return openEmbed(_that.toolId);}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String toolId)?  dispatchImmediate,TResult? Function( String toolId)?  openModal,TResult? Function( String toolId)?  openEmbed,}) {final _that = this;
switch (_that) {
case PinActivationDto_DispatchImmediate() when dispatchImmediate != null:
return dispatchImmediate(_that.toolId);case PinActivationDto_OpenModal() when openModal != null:
return openModal(_that.toolId);case PinActivationDto_OpenEmbed() when openEmbed != null:
return openEmbed(_that.toolId);case _:
  return null;

}
}

}

/// @nodoc


class PinActivationDto_DispatchImmediate extends PinActivationDto {
  const PinActivationDto_DispatchImmediate({required this.toolId}): super._();


@override final  String toolId;

/// Create a copy of PinActivationDto
/// with the given fields replaced by the non-null parameter values.
@override @JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$PinActivationDto_DispatchImmediateCopyWith<PinActivationDto_DispatchImmediate> get copyWith => _$PinActivationDto_DispatchImmediateCopyWithImpl<PinActivationDto_DispatchImmediate>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is PinActivationDto_DispatchImmediate&&(identical(other.toolId, toolId) || other.toolId == toolId));
}


@override
int get hashCode => Object.hash(runtimeType,toolId);

@override
String toString() {
  return 'PinActivationDto.dispatchImmediate(toolId: $toolId)';
}


}

/// @nodoc
abstract mixin class $PinActivationDto_DispatchImmediateCopyWith<$Res> implements $PinActivationDtoCopyWith<$Res> {
  factory $PinActivationDto_DispatchImmediateCopyWith(PinActivationDto_DispatchImmediate value, $Res Function(PinActivationDto_DispatchImmediate) _then) = _$PinActivationDto_DispatchImmediateCopyWithImpl;
@override @useResult
$Res call({
 String toolId
});




}
/// @nodoc
class _$PinActivationDto_DispatchImmediateCopyWithImpl<$Res>
    implements $PinActivationDto_DispatchImmediateCopyWith<$Res> {
  _$PinActivationDto_DispatchImmediateCopyWithImpl(this._self, this._then);

  final PinActivationDto_DispatchImmediate _self;
  final $Res Function(PinActivationDto_DispatchImmediate) _then;

/// Create a copy of PinActivationDto
/// with the given fields replaced by the non-null parameter values.
@override @pragma('vm:prefer-inline') $Res call({Object? toolId = null,}) {
  return _then(PinActivationDto_DispatchImmediate(
toolId: null == toolId ? _self.toolId : toolId // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class PinActivationDto_OpenModal extends PinActivationDto {
  const PinActivationDto_OpenModal({required this.toolId}): super._();


@override final  String toolId;

/// Create a copy of PinActivationDto
/// with the given fields replaced by the non-null parameter values.
@override @JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$PinActivationDto_OpenModalCopyWith<PinActivationDto_OpenModal> get copyWith => _$PinActivationDto_OpenModalCopyWithImpl<PinActivationDto_OpenModal>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is PinActivationDto_OpenModal&&(identical(other.toolId, toolId) || other.toolId == toolId));
}


@override
int get hashCode => Object.hash(runtimeType,toolId);

@override
String toString() {
  return 'PinActivationDto.openModal(toolId: $toolId)';
}


}

/// @nodoc
abstract mixin class $PinActivationDto_OpenModalCopyWith<$Res> implements $PinActivationDtoCopyWith<$Res> {
  factory $PinActivationDto_OpenModalCopyWith(PinActivationDto_OpenModal value, $Res Function(PinActivationDto_OpenModal) _then) = _$PinActivationDto_OpenModalCopyWithImpl;
@override @useResult
$Res call({
 String toolId
});




}
/// @nodoc
class _$PinActivationDto_OpenModalCopyWithImpl<$Res>
    implements $PinActivationDto_OpenModalCopyWith<$Res> {
  _$PinActivationDto_OpenModalCopyWithImpl(this._self, this._then);

  final PinActivationDto_OpenModal _self;
  final $Res Function(PinActivationDto_OpenModal) _then;

/// Create a copy of PinActivationDto
/// with the given fields replaced by the non-null parameter values.
@override @pragma('vm:prefer-inline') $Res call({Object? toolId = null,}) {
  return _then(PinActivationDto_OpenModal(
toolId: null == toolId ? _self.toolId : toolId // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class PinActivationDto_OpenEmbed extends PinActivationDto {
  const PinActivationDto_OpenEmbed({required this.toolId}): super._();


@override final  String toolId;

/// Create a copy of PinActivationDto
/// with the given fields replaced by the non-null parameter values.
@override @JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$PinActivationDto_OpenEmbedCopyWith<PinActivationDto_OpenEmbed> get copyWith => _$PinActivationDto_OpenEmbedCopyWithImpl<PinActivationDto_OpenEmbed>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is PinActivationDto_OpenEmbed&&(identical(other.toolId, toolId) || other.toolId == toolId));
}


@override
int get hashCode => Object.hash(runtimeType,toolId);

@override
String toString() {
  return 'PinActivationDto.openEmbed(toolId: $toolId)';
}


}

/// @nodoc
abstract mixin class $PinActivationDto_OpenEmbedCopyWith<$Res> implements $PinActivationDtoCopyWith<$Res> {
  factory $PinActivationDto_OpenEmbedCopyWith(PinActivationDto_OpenEmbed value, $Res Function(PinActivationDto_OpenEmbed) _then) = _$PinActivationDto_OpenEmbedCopyWithImpl;
@override @useResult
$Res call({
 String toolId
});




}
/// @nodoc
class _$PinActivationDto_OpenEmbedCopyWithImpl<$Res>
    implements $PinActivationDto_OpenEmbedCopyWith<$Res> {
  _$PinActivationDto_OpenEmbedCopyWithImpl(this._self, this._then);

  final PinActivationDto_OpenEmbed _self;
  final $Res Function(PinActivationDto_OpenEmbed) _then;

/// Create a copy of PinActivationDto
/// with the given fields replaced by the non-null parameter values.
@override @pragma('vm:prefer-inline') $Res call({Object? toolId = null,}) {
  return _then(PinActivationDto_OpenEmbed(
toolId: null == toolId ? _self.toolId : toolId // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

// dart format on
