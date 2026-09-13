// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'dispatch_stream.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// dart format off
T _$identity<T>(T value) => value;
/// @nodoc
mixin _$DispatchStreamEventDto {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is DispatchStreamEventDto);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'DispatchStreamEventDto()';
}


}

/// @nodoc
class $DispatchStreamEventDtoCopyWith<$Res>  {
$DispatchStreamEventDtoCopyWith(DispatchStreamEventDto _, $Res Function(DispatchStreamEventDto) __);
}


/// Adds pattern-matching-related methods to [DispatchStreamEventDto].
extension DispatchStreamEventDtoPatterns on DispatchStreamEventDto {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( DispatchStreamEventDto_Chunk value)?  chunk,TResult Function( DispatchStreamEventDto_Done value)?  done,required TResult orElse(),}){
final _that = this;
switch (_that) {
case DispatchStreamEventDto_Chunk() when chunk != null:
return chunk(_that);case DispatchStreamEventDto_Done() when done != null:
return done(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( DispatchStreamEventDto_Chunk value)  chunk,required TResult Function( DispatchStreamEventDto_Done value)  done,}){
final _that = this;
switch (_that) {
case DispatchStreamEventDto_Chunk():
return chunk(_that);case DispatchStreamEventDto_Done():
return done(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( DispatchStreamEventDto_Chunk value)?  chunk,TResult? Function( DispatchStreamEventDto_Done value)?  done,}){
final _that = this;
switch (_that) {
case DispatchStreamEventDto_Chunk() when chunk != null:
return chunk(_that);case DispatchStreamEventDto_Done() when done != null:
return done(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String stream,  BigInt seq,  String chunk)?  chunk,TResult Function( CanonicalToolResult result)?  done,required TResult orElse(),}) {final _that = this;
switch (_that) {
case DispatchStreamEventDto_Chunk() when chunk != null:
return chunk(_that.stream,_that.seq,_that.chunk);case DispatchStreamEventDto_Done() when done != null:
return done(_that.result);case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String stream,  BigInt seq,  String chunk)  chunk,required TResult Function( CanonicalToolResult result)  done,}) {final _that = this;
switch (_that) {
case DispatchStreamEventDto_Chunk():
return chunk(_that.stream,_that.seq,_that.chunk);case DispatchStreamEventDto_Done():
return done(_that.result);}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String stream,  BigInt seq,  String chunk)?  chunk,TResult? Function( CanonicalToolResult result)?  done,}) {final _that = this;
switch (_that) {
case DispatchStreamEventDto_Chunk() when chunk != null:
return chunk(_that.stream,_that.seq,_that.chunk);case DispatchStreamEventDto_Done() when done != null:
return done(_that.result);case _:
  return null;

}
}

}

/// @nodoc


class DispatchStreamEventDto_Chunk extends DispatchStreamEventDto {
  const DispatchStreamEventDto_Chunk({required this.stream, required this.seq, required this.chunk}): super._();


 final  String stream;
 final  BigInt seq;
 final  String chunk;

/// Create a copy of DispatchStreamEventDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$DispatchStreamEventDto_ChunkCopyWith<DispatchStreamEventDto_Chunk> get copyWith => _$DispatchStreamEventDto_ChunkCopyWithImpl<DispatchStreamEventDto_Chunk>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is DispatchStreamEventDto_Chunk&&(identical(other.stream, stream) || other.stream == stream)&&(identical(other.seq, seq) || other.seq == seq)&&(identical(other.chunk, chunk) || other.chunk == chunk));
}


@override
int get hashCode => Object.hash(runtimeType,stream,seq,chunk);

@override
String toString() {
  return 'DispatchStreamEventDto.chunk(stream: $stream, seq: $seq, chunk: $chunk)';
}


}

/// @nodoc
abstract mixin class $DispatchStreamEventDto_ChunkCopyWith<$Res> implements $DispatchStreamEventDtoCopyWith<$Res> {
  factory $DispatchStreamEventDto_ChunkCopyWith(DispatchStreamEventDto_Chunk value, $Res Function(DispatchStreamEventDto_Chunk) _then) = _$DispatchStreamEventDto_ChunkCopyWithImpl;
@useResult
$Res call({
 String stream, BigInt seq, String chunk
});




}
/// @nodoc
class _$DispatchStreamEventDto_ChunkCopyWithImpl<$Res>
    implements $DispatchStreamEventDto_ChunkCopyWith<$Res> {
  _$DispatchStreamEventDto_ChunkCopyWithImpl(this._self, this._then);

  final DispatchStreamEventDto_Chunk _self;
  final $Res Function(DispatchStreamEventDto_Chunk) _then;

/// Create a copy of DispatchStreamEventDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? stream = null,Object? seq = null,Object? chunk = null,}) {
  return _then(DispatchStreamEventDto_Chunk(
stream: null == stream ? _self.stream : stream // ignore: cast_nullable_to_non_nullable
as String,seq: null == seq ? _self.seq : seq // ignore: cast_nullable_to_non_nullable
as BigInt,chunk: null == chunk ? _self.chunk : chunk // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class DispatchStreamEventDto_Done extends DispatchStreamEventDto {
  const DispatchStreamEventDto_Done({required this.result}): super._();


 final  CanonicalToolResult result;

/// Create a copy of DispatchStreamEventDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$DispatchStreamEventDto_DoneCopyWith<DispatchStreamEventDto_Done> get copyWith => _$DispatchStreamEventDto_DoneCopyWithImpl<DispatchStreamEventDto_Done>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is DispatchStreamEventDto_Done&&(identical(other.result, result) || other.result == result));
}


@override
int get hashCode => Object.hash(runtimeType,result);

@override
String toString() {
  return 'DispatchStreamEventDto.done(result: $result)';
}


}

/// @nodoc
abstract mixin class $DispatchStreamEventDto_DoneCopyWith<$Res> implements $DispatchStreamEventDtoCopyWith<$Res> {
  factory $DispatchStreamEventDto_DoneCopyWith(DispatchStreamEventDto_Done value, $Res Function(DispatchStreamEventDto_Done) _then) = _$DispatchStreamEventDto_DoneCopyWithImpl;
@useResult
$Res call({
 CanonicalToolResult result
});




}
/// @nodoc
class _$DispatchStreamEventDto_DoneCopyWithImpl<$Res>
    implements $DispatchStreamEventDto_DoneCopyWith<$Res> {
  _$DispatchStreamEventDto_DoneCopyWithImpl(this._self, this._then);

  final DispatchStreamEventDto_Done _self;
  final $Res Function(DispatchStreamEventDto_Done) _then;

/// Create a copy of DispatchStreamEventDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? result = null,}) {
  return _then(DispatchStreamEventDto_Done(
result: null == result ? _self.result : result // ignore: cast_nullable_to_non_nullable
as CanonicalToolResult,
  ));
}


}

// dart format on
