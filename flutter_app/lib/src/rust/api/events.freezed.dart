// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'events.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// dart format off
T _$identity<T>(T value) => value;
/// @nodoc
mixin _$HostStateEvent {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HostStateEvent);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'HostStateEvent()';
}


}

/// @nodoc
class $HostStateEventCopyWith<$Res>  {
$HostStateEventCopyWith(HostStateEvent _, $Res Function(HostStateEvent) __);
}


/// Adds pattern-matching-related methods to [HostStateEvent].
extension HostStateEventPatterns on HostStateEvent {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( HostStateEvent_Attached value)?  attached,TResult Function( HostStateEvent_Embedded value)?  embedded,TResult Function( HostStateEvent_NoHost value)?  noHost,TResult Function( HostStateEvent_Failed value)?  failed,required TResult orElse(),}){
final _that = this;
switch (_that) {
case HostStateEvent_Attached() when attached != null:
return attached(_that);case HostStateEvent_Embedded() when embedded != null:
return embedded(_that);case HostStateEvent_NoHost() when noHost != null:
return noHost(_that);case HostStateEvent_Failed() when failed != null:
return failed(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( HostStateEvent_Attached value)  attached,required TResult Function( HostStateEvent_Embedded value)  embedded,required TResult Function( HostStateEvent_NoHost value)  noHost,required TResult Function( HostStateEvent_Failed value)  failed,}){
final _that = this;
switch (_that) {
case HostStateEvent_Attached():
return attached(_that);case HostStateEvent_Embedded():
return embedded(_that);case HostStateEvent_NoHost():
return noHost(_that);case HostStateEvent_Failed():
return failed(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( HostStateEvent_Attached value)?  attached,TResult? Function( HostStateEvent_Embedded value)?  embedded,TResult? Function( HostStateEvent_NoHost value)?  noHost,TResult? Function( HostStateEvent_Failed value)?  failed,}){
final _that = this;
switch (_that) {
case HostStateEvent_Attached() when attached != null:
return attached(_that);case HostStateEvent_Embedded() when embedded != null:
return embedded(_that);case HostStateEvent_NoHost() when noHost != null:
return noHost(_that);case HostStateEvent_Failed() when failed != null:
return failed(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String endpoint)?  attached,TResult Function( String endpoint)?  embedded,TResult Function()?  noHost,TResult Function( String reason)?  failed,required TResult orElse(),}) {final _that = this;
switch (_that) {
case HostStateEvent_Attached() when attached != null:
return attached(_that.endpoint);case HostStateEvent_Embedded() when embedded != null:
return embedded(_that.endpoint);case HostStateEvent_NoHost() when noHost != null:
return noHost();case HostStateEvent_Failed() when failed != null:
return failed(_that.reason);case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String endpoint)  attached,required TResult Function( String endpoint)  embedded,required TResult Function()  noHost,required TResult Function( String reason)  failed,}) {final _that = this;
switch (_that) {
case HostStateEvent_Attached():
return attached(_that.endpoint);case HostStateEvent_Embedded():
return embedded(_that.endpoint);case HostStateEvent_NoHost():
return noHost();case HostStateEvent_Failed():
return failed(_that.reason);}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String endpoint)?  attached,TResult? Function( String endpoint)?  embedded,TResult? Function()?  noHost,TResult? Function( String reason)?  failed,}) {final _that = this;
switch (_that) {
case HostStateEvent_Attached() when attached != null:
return attached(_that.endpoint);case HostStateEvent_Embedded() when embedded != null:
return embedded(_that.endpoint);case HostStateEvent_NoHost() when noHost != null:
return noHost();case HostStateEvent_Failed() when failed != null:
return failed(_that.reason);case _:
  return null;

}
}

}

/// @nodoc


class HostStateEvent_Attached extends HostStateEvent {
  const HostStateEvent_Attached({required this.endpoint}): super._();


 final  String endpoint;

/// Create a copy of HostStateEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$HostStateEvent_AttachedCopyWith<HostStateEvent_Attached> get copyWith => _$HostStateEvent_AttachedCopyWithImpl<HostStateEvent_Attached>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HostStateEvent_Attached&&(identical(other.endpoint, endpoint) || other.endpoint == endpoint));
}


@override
int get hashCode => Object.hash(runtimeType,endpoint);

@override
String toString() {
  return 'HostStateEvent.attached(endpoint: $endpoint)';
}


}

/// @nodoc
abstract mixin class $HostStateEvent_AttachedCopyWith<$Res> implements $HostStateEventCopyWith<$Res> {
  factory $HostStateEvent_AttachedCopyWith(HostStateEvent_Attached value, $Res Function(HostStateEvent_Attached) _then) = _$HostStateEvent_AttachedCopyWithImpl;
@useResult
$Res call({
 String endpoint
});




}
/// @nodoc
class _$HostStateEvent_AttachedCopyWithImpl<$Res>
    implements $HostStateEvent_AttachedCopyWith<$Res> {
  _$HostStateEvent_AttachedCopyWithImpl(this._self, this._then);

  final HostStateEvent_Attached _self;
  final $Res Function(HostStateEvent_Attached) _then;

/// Create a copy of HostStateEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? endpoint = null,}) {
  return _then(HostStateEvent_Attached(
endpoint: null == endpoint ? _self.endpoint : endpoint // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class HostStateEvent_Embedded extends HostStateEvent {
  const HostStateEvent_Embedded({required this.endpoint}): super._();


 final  String endpoint;

/// Create a copy of HostStateEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$HostStateEvent_EmbeddedCopyWith<HostStateEvent_Embedded> get copyWith => _$HostStateEvent_EmbeddedCopyWithImpl<HostStateEvent_Embedded>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HostStateEvent_Embedded&&(identical(other.endpoint, endpoint) || other.endpoint == endpoint));
}


@override
int get hashCode => Object.hash(runtimeType,endpoint);

@override
String toString() {
  return 'HostStateEvent.embedded(endpoint: $endpoint)';
}


}

/// @nodoc
abstract mixin class $HostStateEvent_EmbeddedCopyWith<$Res> implements $HostStateEventCopyWith<$Res> {
  factory $HostStateEvent_EmbeddedCopyWith(HostStateEvent_Embedded value, $Res Function(HostStateEvent_Embedded) _then) = _$HostStateEvent_EmbeddedCopyWithImpl;
@useResult
$Res call({
 String endpoint
});




}
/// @nodoc
class _$HostStateEvent_EmbeddedCopyWithImpl<$Res>
    implements $HostStateEvent_EmbeddedCopyWith<$Res> {
  _$HostStateEvent_EmbeddedCopyWithImpl(this._self, this._then);

  final HostStateEvent_Embedded _self;
  final $Res Function(HostStateEvent_Embedded) _then;

/// Create a copy of HostStateEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? endpoint = null,}) {
  return _then(HostStateEvent_Embedded(
endpoint: null == endpoint ? _self.endpoint : endpoint // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class HostStateEvent_NoHost extends HostStateEvent {
  const HostStateEvent_NoHost(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HostStateEvent_NoHost);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'HostStateEvent.noHost()';
}


}




/// @nodoc


class HostStateEvent_Failed extends HostStateEvent {
  const HostStateEvent_Failed({required this.reason}): super._();


 final  String reason;

/// Create a copy of HostStateEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$HostStateEvent_FailedCopyWith<HostStateEvent_Failed> get copyWith => _$HostStateEvent_FailedCopyWithImpl<HostStateEvent_Failed>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HostStateEvent_Failed&&(identical(other.reason, reason) || other.reason == reason));
}


@override
int get hashCode => Object.hash(runtimeType,reason);

@override
String toString() {
  return 'HostStateEvent.failed(reason: $reason)';
}


}

/// @nodoc
abstract mixin class $HostStateEvent_FailedCopyWith<$Res> implements $HostStateEventCopyWith<$Res> {
  factory $HostStateEvent_FailedCopyWith(HostStateEvent_Failed value, $Res Function(HostStateEvent_Failed) _then) = _$HostStateEvent_FailedCopyWithImpl;
@useResult
$Res call({
 String reason
});




}
/// @nodoc
class _$HostStateEvent_FailedCopyWithImpl<$Res>
    implements $HostStateEvent_FailedCopyWith<$Res> {
  _$HostStateEvent_FailedCopyWithImpl(this._self, this._then);

  final HostStateEvent_Failed _self;
  final $Res Function(HostStateEvent_Failed) _then;

/// Create a copy of HostStateEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? reason = null,}) {
  return _then(HostStateEvent_Failed(
reason: null == reason ? _self.reason : reason // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

// dart format on
