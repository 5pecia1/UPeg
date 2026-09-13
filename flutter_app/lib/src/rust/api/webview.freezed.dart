// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'webview.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// dart format off
T _$identity<T>(T value) => value;
/// @nodoc
mixin _$WebViewExecutionCompletionDto {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is WebViewExecutionCompletionDto);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'WebViewExecutionCompletionDto()';
}


}

/// @nodoc
class $WebViewExecutionCompletionDtoCopyWith<$Res>  {
$WebViewExecutionCompletionDtoCopyWith(WebViewExecutionCompletionDto _, $Res Function(WebViewExecutionCompletionDto) __);
}


/// Adds pattern-matching-related methods to [WebViewExecutionCompletionDto].
extension WebViewExecutionCompletionDtoPatterns on WebViewExecutionCompletionDto {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( WebViewExecutionCompletionDto_Success value)?  success,TResult Function( WebViewExecutionCompletionDto_Failed value)?  failed,TResult Function( WebViewExecutionCompletionDto_Cancelled value)?  cancelled,TResult Function( WebViewExecutionCompletionDto_WaitTimeout value)?  waitTimeout,required TResult orElse(),}){
final _that = this;
switch (_that) {
case WebViewExecutionCompletionDto_Success() when success != null:
return success(_that);case WebViewExecutionCompletionDto_Failed() when failed != null:
return failed(_that);case WebViewExecutionCompletionDto_Cancelled() when cancelled != null:
return cancelled(_that);case WebViewExecutionCompletionDto_WaitTimeout() when waitTimeout != null:
return waitTimeout(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( WebViewExecutionCompletionDto_Success value)  success,required TResult Function( WebViewExecutionCompletionDto_Failed value)  failed,required TResult Function( WebViewExecutionCompletionDto_Cancelled value)  cancelled,required TResult Function( WebViewExecutionCompletionDto_WaitTimeout value)  waitTimeout,}){
final _that = this;
switch (_that) {
case WebViewExecutionCompletionDto_Success():
return success(_that);case WebViewExecutionCompletionDto_Failed():
return failed(_that);case WebViewExecutionCompletionDto_Cancelled():
return cancelled(_that);case WebViewExecutionCompletionDto_WaitTimeout():
return waitTimeout(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( WebViewExecutionCompletionDto_Success value)?  success,TResult? Function( WebViewExecutionCompletionDto_Failed value)?  failed,TResult? Function( WebViewExecutionCompletionDto_Cancelled value)?  cancelled,TResult? Function( WebViewExecutionCompletionDto_WaitTimeout value)?  waitTimeout,}){
final _that = this;
switch (_that) {
case WebViewExecutionCompletionDto_Success() when success != null:
return success(_that);case WebViewExecutionCompletionDto_Failed() when failed != null:
return failed(_that);case WebViewExecutionCompletionDto_Cancelled() when cancelled != null:
return cancelled(_that);case WebViewExecutionCompletionDto_WaitTimeout() when waitTimeout != null:
return waitTimeout(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( List<(String, String)> outputs)?  success,TResult Function( String message)?  failed,TResult Function()?  cancelled,TResult Function( BindingRoleDto role,  String selector,  String forSelector,  BindingWaitConditionDto condition,  BigInt timeoutMs)?  waitTimeout,required TResult orElse(),}) {final _that = this;
switch (_that) {
case WebViewExecutionCompletionDto_Success() when success != null:
return success(_that.outputs);case WebViewExecutionCompletionDto_Failed() when failed != null:
return failed(_that.message);case WebViewExecutionCompletionDto_Cancelled() when cancelled != null:
return cancelled();case WebViewExecutionCompletionDto_WaitTimeout() when waitTimeout != null:
return waitTimeout(_that.role,_that.selector,_that.forSelector,_that.condition,_that.timeoutMs);case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( List<(String, String)> outputs)  success,required TResult Function( String message)  failed,required TResult Function()  cancelled,required TResult Function( BindingRoleDto role,  String selector,  String forSelector,  BindingWaitConditionDto condition,  BigInt timeoutMs)  waitTimeout,}) {final _that = this;
switch (_that) {
case WebViewExecutionCompletionDto_Success():
return success(_that.outputs);case WebViewExecutionCompletionDto_Failed():
return failed(_that.message);case WebViewExecutionCompletionDto_Cancelled():
return cancelled();case WebViewExecutionCompletionDto_WaitTimeout():
return waitTimeout(_that.role,_that.selector,_that.forSelector,_that.condition,_that.timeoutMs);}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( List<(String, String)> outputs)?  success,TResult? Function( String message)?  failed,TResult? Function()?  cancelled,TResult? Function( BindingRoleDto role,  String selector,  String forSelector,  BindingWaitConditionDto condition,  BigInt timeoutMs)?  waitTimeout,}) {final _that = this;
switch (_that) {
case WebViewExecutionCompletionDto_Success() when success != null:
return success(_that.outputs);case WebViewExecutionCompletionDto_Failed() when failed != null:
return failed(_that.message);case WebViewExecutionCompletionDto_Cancelled() when cancelled != null:
return cancelled();case WebViewExecutionCompletionDto_WaitTimeout() when waitTimeout != null:
return waitTimeout(_that.role,_that.selector,_that.forSelector,_that.condition,_that.timeoutMs);case _:
  return null;

}
}

}

/// @nodoc


class WebViewExecutionCompletionDto_Success extends WebViewExecutionCompletionDto {
  const WebViewExecutionCompletionDto_Success({required final  List<(String, String)> outputs}): _outputs = outputs,super._();


 final  List<(String, String)> _outputs;
 List<(String, String)> get outputs {
  if (_outputs is EqualUnmodifiableListView) return _outputs;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_outputs);
}


/// Create a copy of WebViewExecutionCompletionDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$WebViewExecutionCompletionDto_SuccessCopyWith<WebViewExecutionCompletionDto_Success> get copyWith => _$WebViewExecutionCompletionDto_SuccessCopyWithImpl<WebViewExecutionCompletionDto_Success>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is WebViewExecutionCompletionDto_Success&&const DeepCollectionEquality().equals(other._outputs, _outputs));
}


@override
int get hashCode => Object.hash(runtimeType,const DeepCollectionEquality().hash(_outputs));

@override
String toString() {
  return 'WebViewExecutionCompletionDto.success(outputs: $outputs)';
}


}

/// @nodoc
abstract mixin class $WebViewExecutionCompletionDto_SuccessCopyWith<$Res> implements $WebViewExecutionCompletionDtoCopyWith<$Res> {
  factory $WebViewExecutionCompletionDto_SuccessCopyWith(WebViewExecutionCompletionDto_Success value, $Res Function(WebViewExecutionCompletionDto_Success) _then) = _$WebViewExecutionCompletionDto_SuccessCopyWithImpl;
@useResult
$Res call({
 List<(String, String)> outputs
});




}
/// @nodoc
class _$WebViewExecutionCompletionDto_SuccessCopyWithImpl<$Res>
    implements $WebViewExecutionCompletionDto_SuccessCopyWith<$Res> {
  _$WebViewExecutionCompletionDto_SuccessCopyWithImpl(this._self, this._then);

  final WebViewExecutionCompletionDto_Success _self;
  final $Res Function(WebViewExecutionCompletionDto_Success) _then;

/// Create a copy of WebViewExecutionCompletionDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? outputs = null,}) {
  return _then(WebViewExecutionCompletionDto_Success(
outputs: null == outputs ? _self._outputs : outputs // ignore: cast_nullable_to_non_nullable
as List<(String, String)>,
  ));
}


}

/// @nodoc


class WebViewExecutionCompletionDto_Failed extends WebViewExecutionCompletionDto {
  const WebViewExecutionCompletionDto_Failed({required this.message}): super._();


 final  String message;

/// Create a copy of WebViewExecutionCompletionDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$WebViewExecutionCompletionDto_FailedCopyWith<WebViewExecutionCompletionDto_Failed> get copyWith => _$WebViewExecutionCompletionDto_FailedCopyWithImpl<WebViewExecutionCompletionDto_Failed>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is WebViewExecutionCompletionDto_Failed&&(identical(other.message, message) || other.message == message));
}


@override
int get hashCode => Object.hash(runtimeType,message);

@override
String toString() {
  return 'WebViewExecutionCompletionDto.failed(message: $message)';
}


}

/// @nodoc
abstract mixin class $WebViewExecutionCompletionDto_FailedCopyWith<$Res> implements $WebViewExecutionCompletionDtoCopyWith<$Res> {
  factory $WebViewExecutionCompletionDto_FailedCopyWith(WebViewExecutionCompletionDto_Failed value, $Res Function(WebViewExecutionCompletionDto_Failed) _then) = _$WebViewExecutionCompletionDto_FailedCopyWithImpl;
@useResult
$Res call({
 String message
});




}
/// @nodoc
class _$WebViewExecutionCompletionDto_FailedCopyWithImpl<$Res>
    implements $WebViewExecutionCompletionDto_FailedCopyWith<$Res> {
  _$WebViewExecutionCompletionDto_FailedCopyWithImpl(this._self, this._then);

  final WebViewExecutionCompletionDto_Failed _self;
  final $Res Function(WebViewExecutionCompletionDto_Failed) _then;

/// Create a copy of WebViewExecutionCompletionDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? message = null,}) {
  return _then(WebViewExecutionCompletionDto_Failed(
message: null == message ? _self.message : message // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class WebViewExecutionCompletionDto_Cancelled extends WebViewExecutionCompletionDto {
  const WebViewExecutionCompletionDto_Cancelled(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is WebViewExecutionCompletionDto_Cancelled);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'WebViewExecutionCompletionDto.cancelled()';
}


}




/// @nodoc


class WebViewExecutionCompletionDto_WaitTimeout extends WebViewExecutionCompletionDto {
  const WebViewExecutionCompletionDto_WaitTimeout({required this.role, required this.selector, required this.forSelector, required this.condition, required this.timeoutMs}): super._();


 final  BindingRoleDto role;
 final  String selector;
 final  String forSelector;
 final  BindingWaitConditionDto condition;
 final  BigInt timeoutMs;

/// Create a copy of WebViewExecutionCompletionDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$WebViewExecutionCompletionDto_WaitTimeoutCopyWith<WebViewExecutionCompletionDto_WaitTimeout> get copyWith => _$WebViewExecutionCompletionDto_WaitTimeoutCopyWithImpl<WebViewExecutionCompletionDto_WaitTimeout>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is WebViewExecutionCompletionDto_WaitTimeout&&(identical(other.role, role) || other.role == role)&&(identical(other.selector, selector) || other.selector == selector)&&(identical(other.forSelector, forSelector) || other.forSelector == forSelector)&&(identical(other.condition, condition) || other.condition == condition)&&(identical(other.timeoutMs, timeoutMs) || other.timeoutMs == timeoutMs));
}


@override
int get hashCode => Object.hash(runtimeType,role,selector,forSelector,condition,timeoutMs);

@override
String toString() {
  return 'WebViewExecutionCompletionDto.waitTimeout(role: $role, selector: $selector, forSelector: $forSelector, condition: $condition, timeoutMs: $timeoutMs)';
}


}

/// @nodoc
abstract mixin class $WebViewExecutionCompletionDto_WaitTimeoutCopyWith<$Res> implements $WebViewExecutionCompletionDtoCopyWith<$Res> {
  factory $WebViewExecutionCompletionDto_WaitTimeoutCopyWith(WebViewExecutionCompletionDto_WaitTimeout value, $Res Function(WebViewExecutionCompletionDto_WaitTimeout) _then) = _$WebViewExecutionCompletionDto_WaitTimeoutCopyWithImpl;
@useResult
$Res call({
 BindingRoleDto role, String selector, String forSelector, BindingWaitConditionDto condition, BigInt timeoutMs
});




}
/// @nodoc
class _$WebViewExecutionCompletionDto_WaitTimeoutCopyWithImpl<$Res>
    implements $WebViewExecutionCompletionDto_WaitTimeoutCopyWith<$Res> {
  _$WebViewExecutionCompletionDto_WaitTimeoutCopyWithImpl(this._self, this._then);

  final WebViewExecutionCompletionDto_WaitTimeout _self;
  final $Res Function(WebViewExecutionCompletionDto_WaitTimeout) _then;

/// Create a copy of WebViewExecutionCompletionDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? role = null,Object? selector = null,Object? forSelector = null,Object? condition = null,Object? timeoutMs = null,}) {
  return _then(WebViewExecutionCompletionDto_WaitTimeout(
role: null == role ? _self.role : role // ignore: cast_nullable_to_non_nullable
as BindingRoleDto,selector: null == selector ? _self.selector : selector // ignore: cast_nullable_to_non_nullable
as String,forSelector: null == forSelector ? _self.forSelector : forSelector // ignore: cast_nullable_to_non_nullable
as String,condition: null == condition ? _self.condition : condition // ignore: cast_nullable_to_non_nullable
as BindingWaitConditionDto,timeoutMs: null == timeoutMs ? _self.timeoutMs : timeoutMs // ignore: cast_nullable_to_non_nullable
as BigInt,
  ));
}


}

/// @nodoc
mixin _$WebViewExecutionEventDto {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is WebViewExecutionEventDto);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'WebViewExecutionEventDto()';
}


}

/// @nodoc
class $WebViewExecutionEventDtoCopyWith<$Res>  {
$WebViewExecutionEventDtoCopyWith(WebViewExecutionEventDto _, $Res Function(WebViewExecutionEventDto) __);
}


/// Adds pattern-matching-related methods to [WebViewExecutionEventDto].
extension WebViewExecutionEventDtoPatterns on WebViewExecutionEventDto {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( WebViewExecutionEventDto_Ready value)?  ready,TResult Function( WebViewExecutionEventDto_Execute value)?  execute,TResult Function( WebViewExecutionEventDto_Cancel value)?  cancel,required TResult orElse(),}){
final _that = this;
switch (_that) {
case WebViewExecutionEventDto_Ready() when ready != null:
return ready(_that);case WebViewExecutionEventDto_Execute() when execute != null:
return execute(_that);case WebViewExecutionEventDto_Cancel() when cancel != null:
return cancel(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( WebViewExecutionEventDto_Ready value)  ready,required TResult Function( WebViewExecutionEventDto_Execute value)  execute,required TResult Function( WebViewExecutionEventDto_Cancel value)  cancel,}){
final _that = this;
switch (_that) {
case WebViewExecutionEventDto_Ready():
return ready(_that);case WebViewExecutionEventDto_Execute():
return execute(_that);case WebViewExecutionEventDto_Cancel():
return cancel(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( WebViewExecutionEventDto_Ready value)?  ready,TResult? Function( WebViewExecutionEventDto_Execute value)?  execute,TResult? Function( WebViewExecutionEventDto_Cancel value)?  cancel,}){
final _that = this;
switch (_that) {
case WebViewExecutionEventDto_Ready() when ready != null:
return ready(_that);case WebViewExecutionEventDto_Execute() when execute != null:
return execute(_that);case WebViewExecutionEventDto_Cancel() when cancel != null:
return cancel(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function()?  ready,TResult Function( WebViewExecutionRequestDto request)?  execute,TResult Function( BigInt requestId)?  cancel,required TResult orElse(),}) {final _that = this;
switch (_that) {
case WebViewExecutionEventDto_Ready() when ready != null:
return ready();case WebViewExecutionEventDto_Execute() when execute != null:
return execute(_that.request);case WebViewExecutionEventDto_Cancel() when cancel != null:
return cancel(_that.requestId);case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function()  ready,required TResult Function( WebViewExecutionRequestDto request)  execute,required TResult Function( BigInt requestId)  cancel,}) {final _that = this;
switch (_that) {
case WebViewExecutionEventDto_Ready():
return ready();case WebViewExecutionEventDto_Execute():
return execute(_that.request);case WebViewExecutionEventDto_Cancel():
return cancel(_that.requestId);}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function()?  ready,TResult? Function( WebViewExecutionRequestDto request)?  execute,TResult? Function( BigInt requestId)?  cancel,}) {final _that = this;
switch (_that) {
case WebViewExecutionEventDto_Ready() when ready != null:
return ready();case WebViewExecutionEventDto_Execute() when execute != null:
return execute(_that.request);case WebViewExecutionEventDto_Cancel() when cancel != null:
return cancel(_that.requestId);case _:
  return null;

}
}

}

/// @nodoc


class WebViewExecutionEventDto_Ready extends WebViewExecutionEventDto {
  const WebViewExecutionEventDto_Ready(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is WebViewExecutionEventDto_Ready);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'WebViewExecutionEventDto.ready()';
}


}




/// @nodoc


class WebViewExecutionEventDto_Execute extends WebViewExecutionEventDto {
  const WebViewExecutionEventDto_Execute({required this.request}): super._();


 final  WebViewExecutionRequestDto request;

/// Create a copy of WebViewExecutionEventDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$WebViewExecutionEventDto_ExecuteCopyWith<WebViewExecutionEventDto_Execute> get copyWith => _$WebViewExecutionEventDto_ExecuteCopyWithImpl<WebViewExecutionEventDto_Execute>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is WebViewExecutionEventDto_Execute&&(identical(other.request, request) || other.request == request));
}


@override
int get hashCode => Object.hash(runtimeType,request);

@override
String toString() {
  return 'WebViewExecutionEventDto.execute(request: $request)';
}


}

/// @nodoc
abstract mixin class $WebViewExecutionEventDto_ExecuteCopyWith<$Res> implements $WebViewExecutionEventDtoCopyWith<$Res> {
  factory $WebViewExecutionEventDto_ExecuteCopyWith(WebViewExecutionEventDto_Execute value, $Res Function(WebViewExecutionEventDto_Execute) _then) = _$WebViewExecutionEventDto_ExecuteCopyWithImpl;
@useResult
$Res call({
 WebViewExecutionRequestDto request
});




}
/// @nodoc
class _$WebViewExecutionEventDto_ExecuteCopyWithImpl<$Res>
    implements $WebViewExecutionEventDto_ExecuteCopyWith<$Res> {
  _$WebViewExecutionEventDto_ExecuteCopyWithImpl(this._self, this._then);

  final WebViewExecutionEventDto_Execute _self;
  final $Res Function(WebViewExecutionEventDto_Execute) _then;

/// Create a copy of WebViewExecutionEventDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? request = null,}) {
  return _then(WebViewExecutionEventDto_Execute(
request: null == request ? _self.request : request // ignore: cast_nullable_to_non_nullable
as WebViewExecutionRequestDto,
  ));
}


}

/// @nodoc


class WebViewExecutionEventDto_Cancel extends WebViewExecutionEventDto {
  const WebViewExecutionEventDto_Cancel({required this.requestId}): super._();


 final  BigInt requestId;

/// Create a copy of WebViewExecutionEventDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$WebViewExecutionEventDto_CancelCopyWith<WebViewExecutionEventDto_Cancel> get copyWith => _$WebViewExecutionEventDto_CancelCopyWithImpl<WebViewExecutionEventDto_Cancel>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is WebViewExecutionEventDto_Cancel&&(identical(other.requestId, requestId) || other.requestId == requestId));
}


@override
int get hashCode => Object.hash(runtimeType,requestId);

@override
String toString() {
  return 'WebViewExecutionEventDto.cancel(requestId: $requestId)';
}


}

/// @nodoc
abstract mixin class $WebViewExecutionEventDto_CancelCopyWith<$Res> implements $WebViewExecutionEventDtoCopyWith<$Res> {
  factory $WebViewExecutionEventDto_CancelCopyWith(WebViewExecutionEventDto_Cancel value, $Res Function(WebViewExecutionEventDto_Cancel) _then) = _$WebViewExecutionEventDto_CancelCopyWithImpl;
@useResult
$Res call({
 BigInt requestId
});




}
/// @nodoc
class _$WebViewExecutionEventDto_CancelCopyWithImpl<$Res>
    implements $WebViewExecutionEventDto_CancelCopyWith<$Res> {
  _$WebViewExecutionEventDto_CancelCopyWithImpl(this._self, this._then);

  final WebViewExecutionEventDto_Cancel _self;
  final $Res Function(WebViewExecutionEventDto_Cancel) _then;

/// Create a copy of WebViewExecutionEventDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? requestId = null,}) {
  return _then(WebViewExecutionEventDto_Cancel(
requestId: null == requestId ? _self.requestId : requestId // ignore: cast_nullable_to_non_nullable
as BigInt,
  ));
}


}

// dart format on
