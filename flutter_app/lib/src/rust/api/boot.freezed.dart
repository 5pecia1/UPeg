// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'boot.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// dart format off
T _$identity<T>(T value) => value;
/// @nodoc
mixin _$FrbError {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is FrbError);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'FrbError()';
}


}

/// @nodoc
class $FrbErrorCopyWith<$Res>  {
$FrbErrorCopyWith(FrbError _, $Res Function(FrbError) __);
}


/// Adds pattern-matching-related methods to [FrbError].
extension FrbErrorPatterns on FrbError {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( FrbError_AlreadyRunning value)?  alreadyRunning,TResult Function( FrbError_HostUnavailable value)?  hostUnavailable,TResult Function( FrbError_ProjectManifestChanged value)?  projectManifestChanged,TResult Function( FrbError_Validation value)?  validation,TResult Function( FrbError_Io value)?  io,TResult Function( FrbError_Internal value)?  internal,required TResult orElse(),}){
final _that = this;
switch (_that) {
case FrbError_AlreadyRunning() when alreadyRunning != null:
return alreadyRunning(_that);case FrbError_HostUnavailable() when hostUnavailable != null:
return hostUnavailable(_that);case FrbError_ProjectManifestChanged() when projectManifestChanged != null:
return projectManifestChanged(_that);case FrbError_Validation() when validation != null:
return validation(_that);case FrbError_Io() when io != null:
return io(_that);case FrbError_Internal() when internal != null:
return internal(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( FrbError_AlreadyRunning value)  alreadyRunning,required TResult Function( FrbError_HostUnavailable value)  hostUnavailable,required TResult Function( FrbError_ProjectManifestChanged value)  projectManifestChanged,required TResult Function( FrbError_Validation value)  validation,required TResult Function( FrbError_Io value)  io,required TResult Function( FrbError_Internal value)  internal,}){
final _that = this;
switch (_that) {
case FrbError_AlreadyRunning():
return alreadyRunning(_that);case FrbError_HostUnavailable():
return hostUnavailable(_that);case FrbError_ProjectManifestChanged():
return projectManifestChanged(_that);case FrbError_Validation():
return validation(_that);case FrbError_Io():
return io(_that);case FrbError_Internal():
return internal(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( FrbError_AlreadyRunning value)?  alreadyRunning,TResult? Function( FrbError_HostUnavailable value)?  hostUnavailable,TResult? Function( FrbError_ProjectManifestChanged value)?  projectManifestChanged,TResult? Function( FrbError_Validation value)?  validation,TResult? Function( FrbError_Io value)?  io,TResult? Function( FrbError_Internal value)?  internal,}){
final _that = this;
switch (_that) {
case FrbError_AlreadyRunning() when alreadyRunning != null:
return alreadyRunning(_that);case FrbError_HostUnavailable() when hostUnavailable != null:
return hostUnavailable(_that);case FrbError_ProjectManifestChanged() when projectManifestChanged != null:
return projectManifestChanged(_that);case FrbError_Validation() when validation != null:
return validation(_that);case FrbError_Io() when io != null:
return io(_that);case FrbError_Internal() when internal != null:
return internal(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( int pid)?  alreadyRunning,TResult Function()?  hostUnavailable,TResult Function( String path)?  projectManifestChanged,TResult Function( String field,  String reason)?  validation,TResult Function( String message)?  io,TResult Function( String message)?  internal,required TResult orElse(),}) {final _that = this;
switch (_that) {
case FrbError_AlreadyRunning() when alreadyRunning != null:
return alreadyRunning(_that.pid);case FrbError_HostUnavailable() when hostUnavailable != null:
return hostUnavailable();case FrbError_ProjectManifestChanged() when projectManifestChanged != null:
return projectManifestChanged(_that.path);case FrbError_Validation() when validation != null:
return validation(_that.field,_that.reason);case FrbError_Io() when io != null:
return io(_that.message);case FrbError_Internal() when internal != null:
return internal(_that.message);case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( int pid)  alreadyRunning,required TResult Function()  hostUnavailable,required TResult Function( String path)  projectManifestChanged,required TResult Function( String field,  String reason)  validation,required TResult Function( String message)  io,required TResult Function( String message)  internal,}) {final _that = this;
switch (_that) {
case FrbError_AlreadyRunning():
return alreadyRunning(_that.pid);case FrbError_HostUnavailable():
return hostUnavailable();case FrbError_ProjectManifestChanged():
return projectManifestChanged(_that.path);case FrbError_Validation():
return validation(_that.field,_that.reason);case FrbError_Io():
return io(_that.message);case FrbError_Internal():
return internal(_that.message);}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( int pid)?  alreadyRunning,TResult? Function()?  hostUnavailable,TResult? Function( String path)?  projectManifestChanged,TResult? Function( String field,  String reason)?  validation,TResult? Function( String message)?  io,TResult? Function( String message)?  internal,}) {final _that = this;
switch (_that) {
case FrbError_AlreadyRunning() when alreadyRunning != null:
return alreadyRunning(_that.pid);case FrbError_HostUnavailable() when hostUnavailable != null:
return hostUnavailable();case FrbError_ProjectManifestChanged() when projectManifestChanged != null:
return projectManifestChanged(_that.path);case FrbError_Validation() when validation != null:
return validation(_that.field,_that.reason);case FrbError_Io() when io != null:
return io(_that.message);case FrbError_Internal() when internal != null:
return internal(_that.message);case _:
  return null;

}
}

}

/// @nodoc


class FrbError_AlreadyRunning extends FrbError {
  const FrbError_AlreadyRunning({required this.pid}): super._();


 final  int pid;

/// Create a copy of FrbError
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$FrbError_AlreadyRunningCopyWith<FrbError_AlreadyRunning> get copyWith => _$FrbError_AlreadyRunningCopyWithImpl<FrbError_AlreadyRunning>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is FrbError_AlreadyRunning&&(identical(other.pid, pid) || other.pid == pid));
}


@override
int get hashCode => Object.hash(runtimeType,pid);

@override
String toString() {
  return 'FrbError.alreadyRunning(pid: $pid)';
}


}

/// @nodoc
abstract mixin class $FrbError_AlreadyRunningCopyWith<$Res> implements $FrbErrorCopyWith<$Res> {
  factory $FrbError_AlreadyRunningCopyWith(FrbError_AlreadyRunning value, $Res Function(FrbError_AlreadyRunning) _then) = _$FrbError_AlreadyRunningCopyWithImpl;
@useResult
$Res call({
 int pid
});




}
/// @nodoc
class _$FrbError_AlreadyRunningCopyWithImpl<$Res>
    implements $FrbError_AlreadyRunningCopyWith<$Res> {
  _$FrbError_AlreadyRunningCopyWithImpl(this._self, this._then);

  final FrbError_AlreadyRunning _self;
  final $Res Function(FrbError_AlreadyRunning) _then;

/// Create a copy of FrbError
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? pid = null,}) {
  return _then(FrbError_AlreadyRunning(
pid: null == pid ? _self.pid : pid // ignore: cast_nullable_to_non_nullable
as int,
  ));
}


}

/// @nodoc


class FrbError_HostUnavailable extends FrbError {
  const FrbError_HostUnavailable(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is FrbError_HostUnavailable);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'FrbError.hostUnavailable()';
}


}




/// @nodoc


class FrbError_ProjectManifestChanged extends FrbError {
  const FrbError_ProjectManifestChanged({required this.path}): super._();


 final  String path;

/// Create a copy of FrbError
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$FrbError_ProjectManifestChangedCopyWith<FrbError_ProjectManifestChanged> get copyWith => _$FrbError_ProjectManifestChangedCopyWithImpl<FrbError_ProjectManifestChanged>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is FrbError_ProjectManifestChanged&&(identical(other.path, path) || other.path == path));
}


@override
int get hashCode => Object.hash(runtimeType,path);

@override
String toString() {
  return 'FrbError.projectManifestChanged(path: $path)';
}


}

/// @nodoc
abstract mixin class $FrbError_ProjectManifestChangedCopyWith<$Res> implements $FrbErrorCopyWith<$Res> {
  factory $FrbError_ProjectManifestChangedCopyWith(FrbError_ProjectManifestChanged value, $Res Function(FrbError_ProjectManifestChanged) _then) = _$FrbError_ProjectManifestChangedCopyWithImpl;
@useResult
$Res call({
 String path
});




}
/// @nodoc
class _$FrbError_ProjectManifestChangedCopyWithImpl<$Res>
    implements $FrbError_ProjectManifestChangedCopyWith<$Res> {
  _$FrbError_ProjectManifestChangedCopyWithImpl(this._self, this._then);

  final FrbError_ProjectManifestChanged _self;
  final $Res Function(FrbError_ProjectManifestChanged) _then;

/// Create a copy of FrbError
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? path = null,}) {
  return _then(FrbError_ProjectManifestChanged(
path: null == path ? _self.path : path // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class FrbError_Validation extends FrbError {
  const FrbError_Validation({required this.field, required this.reason}): super._();


 final  String field;
 final  String reason;

/// Create a copy of FrbError
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$FrbError_ValidationCopyWith<FrbError_Validation> get copyWith => _$FrbError_ValidationCopyWithImpl<FrbError_Validation>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is FrbError_Validation&&(identical(other.field, field) || other.field == field)&&(identical(other.reason, reason) || other.reason == reason));
}


@override
int get hashCode => Object.hash(runtimeType,field,reason);

@override
String toString() {
  return 'FrbError.validation(field: $field, reason: $reason)';
}


}

/// @nodoc
abstract mixin class $FrbError_ValidationCopyWith<$Res> implements $FrbErrorCopyWith<$Res> {
  factory $FrbError_ValidationCopyWith(FrbError_Validation value, $Res Function(FrbError_Validation) _then) = _$FrbError_ValidationCopyWithImpl;
@useResult
$Res call({
 String field, String reason
});




}
/// @nodoc
class _$FrbError_ValidationCopyWithImpl<$Res>
    implements $FrbError_ValidationCopyWith<$Res> {
  _$FrbError_ValidationCopyWithImpl(this._self, this._then);

  final FrbError_Validation _self;
  final $Res Function(FrbError_Validation) _then;

/// Create a copy of FrbError
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? field = null,Object? reason = null,}) {
  return _then(FrbError_Validation(
field: null == field ? _self.field : field // ignore: cast_nullable_to_non_nullable
as String,reason: null == reason ? _self.reason : reason // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class FrbError_Io extends FrbError {
  const FrbError_Io({required this.message}): super._();


 final  String message;

/// Create a copy of FrbError
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$FrbError_IoCopyWith<FrbError_Io> get copyWith => _$FrbError_IoCopyWithImpl<FrbError_Io>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is FrbError_Io&&(identical(other.message, message) || other.message == message));
}


@override
int get hashCode => Object.hash(runtimeType,message);

@override
String toString() {
  return 'FrbError.io(message: $message)';
}


}

/// @nodoc
abstract mixin class $FrbError_IoCopyWith<$Res> implements $FrbErrorCopyWith<$Res> {
  factory $FrbError_IoCopyWith(FrbError_Io value, $Res Function(FrbError_Io) _then) = _$FrbError_IoCopyWithImpl;
@useResult
$Res call({
 String message
});




}
/// @nodoc
class _$FrbError_IoCopyWithImpl<$Res>
    implements $FrbError_IoCopyWith<$Res> {
  _$FrbError_IoCopyWithImpl(this._self, this._then);

  final FrbError_Io _self;
  final $Res Function(FrbError_Io) _then;

/// Create a copy of FrbError
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? message = null,}) {
  return _then(FrbError_Io(
message: null == message ? _self.message : message // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class FrbError_Internal extends FrbError {
  const FrbError_Internal({required this.message}): super._();


 final  String message;

/// Create a copy of FrbError
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$FrbError_InternalCopyWith<FrbError_Internal> get copyWith => _$FrbError_InternalCopyWithImpl<FrbError_Internal>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is FrbError_Internal&&(identical(other.message, message) || other.message == message));
}


@override
int get hashCode => Object.hash(runtimeType,message);

@override
String toString() {
  return 'FrbError.internal(message: $message)';
}


}

/// @nodoc
abstract mixin class $FrbError_InternalCopyWith<$Res> implements $FrbErrorCopyWith<$Res> {
  factory $FrbError_InternalCopyWith(FrbError_Internal value, $Res Function(FrbError_Internal) _then) = _$FrbError_InternalCopyWithImpl;
@useResult
$Res call({
 String message
});




}
/// @nodoc
class _$FrbError_InternalCopyWithImpl<$Res>
    implements $FrbError_InternalCopyWith<$Res> {
  _$FrbError_InternalCopyWithImpl(this._self, this._then);

  final FrbError_Internal _self;
  final $Res Function(FrbError_Internal) _then;

/// Create a copy of FrbError
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? message = null,}) {
  return _then(FrbError_Internal(
message: null == message ? _self.message : message // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc
mixin _$HostStateDto {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HostStateDto);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'HostStateDto()';
}


}

/// @nodoc
class $HostStateDtoCopyWith<$Res>  {
$HostStateDtoCopyWith(HostStateDto _, $Res Function(HostStateDto) __);
}


/// Adds pattern-matching-related methods to [HostStateDto].
extension HostStateDtoPatterns on HostStateDto {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( HostStateDto_Attached value)?  attached,TResult Function( HostStateDto_Embedded value)?  embedded,TResult Function( HostStateDto_NoHost value)?  noHost,required TResult orElse(),}){
final _that = this;
switch (_that) {
case HostStateDto_Attached() when attached != null:
return attached(_that);case HostStateDto_Embedded() when embedded != null:
return embedded(_that);case HostStateDto_NoHost() when noHost != null:
return noHost(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( HostStateDto_Attached value)  attached,required TResult Function( HostStateDto_Embedded value)  embedded,required TResult Function( HostStateDto_NoHost value)  noHost,}){
final _that = this;
switch (_that) {
case HostStateDto_Attached():
return attached(_that);case HostStateDto_Embedded():
return embedded(_that);case HostStateDto_NoHost():
return noHost(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( HostStateDto_Attached value)?  attached,TResult? Function( HostStateDto_Embedded value)?  embedded,TResult? Function( HostStateDto_NoHost value)?  noHost,}){
final _that = this;
switch (_that) {
case HostStateDto_Attached() when attached != null:
return attached(_that);case HostStateDto_Embedded() when embedded != null:
return embedded(_that);case HostStateDto_NoHost() when noHost != null:
return noHost(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String endpoint)?  attached,TResult Function( String endpoint)?  embedded,TResult Function()?  noHost,required TResult orElse(),}) {final _that = this;
switch (_that) {
case HostStateDto_Attached() when attached != null:
return attached(_that.endpoint);case HostStateDto_Embedded() when embedded != null:
return embedded(_that.endpoint);case HostStateDto_NoHost() when noHost != null:
return noHost();case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String endpoint)  attached,required TResult Function( String endpoint)  embedded,required TResult Function()  noHost,}) {final _that = this;
switch (_that) {
case HostStateDto_Attached():
return attached(_that.endpoint);case HostStateDto_Embedded():
return embedded(_that.endpoint);case HostStateDto_NoHost():
return noHost();}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String endpoint)?  attached,TResult? Function( String endpoint)?  embedded,TResult? Function()?  noHost,}) {final _that = this;
switch (_that) {
case HostStateDto_Attached() when attached != null:
return attached(_that.endpoint);case HostStateDto_Embedded() when embedded != null:
return embedded(_that.endpoint);case HostStateDto_NoHost() when noHost != null:
return noHost();case _:
  return null;

}
}

}

/// @nodoc


class HostStateDto_Attached extends HostStateDto {
  const HostStateDto_Attached({required this.endpoint}): super._();


 final  String endpoint;

/// Create a copy of HostStateDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$HostStateDto_AttachedCopyWith<HostStateDto_Attached> get copyWith => _$HostStateDto_AttachedCopyWithImpl<HostStateDto_Attached>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HostStateDto_Attached&&(identical(other.endpoint, endpoint) || other.endpoint == endpoint));
}


@override
int get hashCode => Object.hash(runtimeType,endpoint);

@override
String toString() {
  return 'HostStateDto.attached(endpoint: $endpoint)';
}


}

/// @nodoc
abstract mixin class $HostStateDto_AttachedCopyWith<$Res> implements $HostStateDtoCopyWith<$Res> {
  factory $HostStateDto_AttachedCopyWith(HostStateDto_Attached value, $Res Function(HostStateDto_Attached) _then) = _$HostStateDto_AttachedCopyWithImpl;
@useResult
$Res call({
 String endpoint
});




}
/// @nodoc
class _$HostStateDto_AttachedCopyWithImpl<$Res>
    implements $HostStateDto_AttachedCopyWith<$Res> {
  _$HostStateDto_AttachedCopyWithImpl(this._self, this._then);

  final HostStateDto_Attached _self;
  final $Res Function(HostStateDto_Attached) _then;

/// Create a copy of HostStateDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? endpoint = null,}) {
  return _then(HostStateDto_Attached(
endpoint: null == endpoint ? _self.endpoint : endpoint // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class HostStateDto_Embedded extends HostStateDto {
  const HostStateDto_Embedded({required this.endpoint}): super._();


 final  String endpoint;

/// Create a copy of HostStateDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$HostStateDto_EmbeddedCopyWith<HostStateDto_Embedded> get copyWith => _$HostStateDto_EmbeddedCopyWithImpl<HostStateDto_Embedded>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HostStateDto_Embedded&&(identical(other.endpoint, endpoint) || other.endpoint == endpoint));
}


@override
int get hashCode => Object.hash(runtimeType,endpoint);

@override
String toString() {
  return 'HostStateDto.embedded(endpoint: $endpoint)';
}


}

/// @nodoc
abstract mixin class $HostStateDto_EmbeddedCopyWith<$Res> implements $HostStateDtoCopyWith<$Res> {
  factory $HostStateDto_EmbeddedCopyWith(HostStateDto_Embedded value, $Res Function(HostStateDto_Embedded) _then) = _$HostStateDto_EmbeddedCopyWithImpl;
@useResult
$Res call({
 String endpoint
});




}
/// @nodoc
class _$HostStateDto_EmbeddedCopyWithImpl<$Res>
    implements $HostStateDto_EmbeddedCopyWith<$Res> {
  _$HostStateDto_EmbeddedCopyWithImpl(this._self, this._then);

  final HostStateDto_Embedded _self;
  final $Res Function(HostStateDto_Embedded) _then;

/// Create a copy of HostStateDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? endpoint = null,}) {
  return _then(HostStateDto_Embedded(
endpoint: null == endpoint ? _self.endpoint : endpoint // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class HostStateDto_NoHost extends HostStateDto {
  const HostStateDto_NoHost(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HostStateDto_NoHost);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'HostStateDto.noHost()';
}


}




// dart format on
