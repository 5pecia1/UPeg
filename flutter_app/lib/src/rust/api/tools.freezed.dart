// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'tools.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// dart format off
T _$identity<T>(T value) => value;
/// @nodoc
mixin _$CanonicalFileContent {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CanonicalFileContent);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'CanonicalFileContent()';
}


}

/// @nodoc
class $CanonicalFileContentCopyWith<$Res>  {
$CanonicalFileContentCopyWith(CanonicalFileContent _, $Res Function(CanonicalFileContent) __);
}


/// Adds pattern-matching-related methods to [CanonicalFileContent].
extension CanonicalFileContentPatterns on CanonicalFileContent {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( CanonicalFileContent_Bytes value)?  bytes,TResult Function( CanonicalFileContent_Directory value)?  directory,required TResult orElse(),}){
final _that = this;
switch (_that) {
case CanonicalFileContent_Bytes() when bytes != null:
return bytes(_that);case CanonicalFileContent_Directory() when directory != null:
return directory(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( CanonicalFileContent_Bytes value)  bytes,required TResult Function( CanonicalFileContent_Directory value)  directory,}){
final _that = this;
switch (_that) {
case CanonicalFileContent_Bytes():
return bytes(_that);case CanonicalFileContent_Directory():
return directory(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( CanonicalFileContent_Bytes value)?  bytes,TResult? Function( CanonicalFileContent_Directory value)?  directory,}){
final _that = this;
switch (_that) {
case CanonicalFileContent_Bytes() when bytes != null:
return bytes(_that);case CanonicalFileContent_Directory() when directory != null:
return directory(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( Uint8List bytes)?  bytes,TResult Function( List<CanonicalFileValue> entries)?  directory,required TResult orElse(),}) {final _that = this;
switch (_that) {
case CanonicalFileContent_Bytes() when bytes != null:
return bytes(_that.bytes);case CanonicalFileContent_Directory() when directory != null:
return directory(_that.entries);case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( Uint8List bytes)  bytes,required TResult Function( List<CanonicalFileValue> entries)  directory,}) {final _that = this;
switch (_that) {
case CanonicalFileContent_Bytes():
return bytes(_that.bytes);case CanonicalFileContent_Directory():
return directory(_that.entries);}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( Uint8List bytes)?  bytes,TResult? Function( List<CanonicalFileValue> entries)?  directory,}) {final _that = this;
switch (_that) {
case CanonicalFileContent_Bytes() when bytes != null:
return bytes(_that.bytes);case CanonicalFileContent_Directory() when directory != null:
return directory(_that.entries);case _:
  return null;

}
}

}

/// @nodoc


class CanonicalFileContent_Bytes extends CanonicalFileContent {
  const CanonicalFileContent_Bytes({required this.bytes}): super._();


 final  Uint8List bytes;

/// Create a copy of CanonicalFileContent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CanonicalFileContent_BytesCopyWith<CanonicalFileContent_Bytes> get copyWith => _$CanonicalFileContent_BytesCopyWithImpl<CanonicalFileContent_Bytes>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CanonicalFileContent_Bytes&&const DeepCollectionEquality().equals(other.bytes, bytes));
}


@override
int get hashCode => Object.hash(runtimeType,const DeepCollectionEquality().hash(bytes));

@override
String toString() {
  return 'CanonicalFileContent.bytes(bytes: $bytes)';
}


}

/// @nodoc
abstract mixin class $CanonicalFileContent_BytesCopyWith<$Res> implements $CanonicalFileContentCopyWith<$Res> {
  factory $CanonicalFileContent_BytesCopyWith(CanonicalFileContent_Bytes value, $Res Function(CanonicalFileContent_Bytes) _then) = _$CanonicalFileContent_BytesCopyWithImpl;
@useResult
$Res call({
 Uint8List bytes
});




}
/// @nodoc
class _$CanonicalFileContent_BytesCopyWithImpl<$Res>
    implements $CanonicalFileContent_BytesCopyWith<$Res> {
  _$CanonicalFileContent_BytesCopyWithImpl(this._self, this._then);

  final CanonicalFileContent_Bytes _self;
  final $Res Function(CanonicalFileContent_Bytes) _then;

/// Create a copy of CanonicalFileContent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? bytes = null,}) {
  return _then(CanonicalFileContent_Bytes(
bytes: null == bytes ? _self.bytes : bytes // ignore: cast_nullable_to_non_nullable
as Uint8List,
  ));
}


}

/// @nodoc


class CanonicalFileContent_Directory extends CanonicalFileContent {
  const CanonicalFileContent_Directory({required final  List<CanonicalFileValue> entries}): _entries = entries,super._();


 final  List<CanonicalFileValue> _entries;
 List<CanonicalFileValue> get entries {
  if (_entries is EqualUnmodifiableListView) return _entries;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_entries);
}


/// Create a copy of CanonicalFileContent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CanonicalFileContent_DirectoryCopyWith<CanonicalFileContent_Directory> get copyWith => _$CanonicalFileContent_DirectoryCopyWithImpl<CanonicalFileContent_Directory>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CanonicalFileContent_Directory&&const DeepCollectionEquality().equals(other._entries, _entries));
}


@override
int get hashCode => Object.hash(runtimeType,const DeepCollectionEquality().hash(_entries));

@override
String toString() {
  return 'CanonicalFileContent.directory(entries: $entries)';
}


}

/// @nodoc
abstract mixin class $CanonicalFileContent_DirectoryCopyWith<$Res> implements $CanonicalFileContentCopyWith<$Res> {
  factory $CanonicalFileContent_DirectoryCopyWith(CanonicalFileContent_Directory value, $Res Function(CanonicalFileContent_Directory) _then) = _$CanonicalFileContent_DirectoryCopyWithImpl;
@useResult
$Res call({
 List<CanonicalFileValue> entries
});




}
/// @nodoc
class _$CanonicalFileContent_DirectoryCopyWithImpl<$Res>
    implements $CanonicalFileContent_DirectoryCopyWith<$Res> {
  _$CanonicalFileContent_DirectoryCopyWithImpl(this._self, this._then);

  final CanonicalFileContent_Directory _self;
  final $Res Function(CanonicalFileContent_Directory) _then;

/// Create a copy of CanonicalFileContent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? entries = null,}) {
  return _then(CanonicalFileContent_Directory(
entries: null == entries ? _self._entries : entries // ignore: cast_nullable_to_non_nullable
as List<CanonicalFileValue>,
  ));
}


}

/// @nodoc
mixin _$CanonicalOutputValue {

 Object get value;



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CanonicalOutputValue&&const DeepCollectionEquality().equals(other.value, value));
}


@override
int get hashCode => Object.hash(runtimeType,const DeepCollectionEquality().hash(value));

@override
String toString() {
  return 'CanonicalOutputValue(value: $value)';
}


}

/// @nodoc
class $CanonicalOutputValueCopyWith<$Res>  {
$CanonicalOutputValueCopyWith(CanonicalOutputValue _, $Res Function(CanonicalOutputValue) __);
}


/// Adds pattern-matching-related methods to [CanonicalOutputValue].
extension CanonicalOutputValuePatterns on CanonicalOutputValue {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( CanonicalOutputValue_String value)?  string,TResult Function( CanonicalOutputValue_Number value)?  number,TResult Function( CanonicalOutputValue_Integer value)?  integer,TResult Function( CanonicalOutputValue_Boolean value)?  boolean,TResult Function( CanonicalOutputValue_Options value)?  options,TResult Function( CanonicalOutputValue_MultiOptions value)?  multiOptions,TResult Function( CanonicalOutputValue_Markdown value)?  markdown,TResult Function( CanonicalOutputValue_Json value)?  json,TResult Function( CanonicalOutputValue_DateTime value)?  dateTime,TResult Function( CanonicalOutputValue_FilePath value)?  filePath,TResult Function( CanonicalOutputValue_Url value)?  url,TResult Function( CanonicalOutputValue_File value)?  file,TResult Function( CanonicalOutputValue_EmbeddedView value)?  embeddedView,required TResult orElse(),}){
final _that = this;
switch (_that) {
case CanonicalOutputValue_String() when string != null:
return string(_that);case CanonicalOutputValue_Number() when number != null:
return number(_that);case CanonicalOutputValue_Integer() when integer != null:
return integer(_that);case CanonicalOutputValue_Boolean() when boolean != null:
return boolean(_that);case CanonicalOutputValue_Options() when options != null:
return options(_that);case CanonicalOutputValue_MultiOptions() when multiOptions != null:
return multiOptions(_that);case CanonicalOutputValue_Markdown() when markdown != null:
return markdown(_that);case CanonicalOutputValue_Json() when json != null:
return json(_that);case CanonicalOutputValue_DateTime() when dateTime != null:
return dateTime(_that);case CanonicalOutputValue_FilePath() when filePath != null:
return filePath(_that);case CanonicalOutputValue_Url() when url != null:
return url(_that);case CanonicalOutputValue_File() when file != null:
return file(_that);case CanonicalOutputValue_EmbeddedView() when embeddedView != null:
return embeddedView(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( CanonicalOutputValue_String value)  string,required TResult Function( CanonicalOutputValue_Number value)  number,required TResult Function( CanonicalOutputValue_Integer value)  integer,required TResult Function( CanonicalOutputValue_Boolean value)  boolean,required TResult Function( CanonicalOutputValue_Options value)  options,required TResult Function( CanonicalOutputValue_MultiOptions value)  multiOptions,required TResult Function( CanonicalOutputValue_Markdown value)  markdown,required TResult Function( CanonicalOutputValue_Json value)  json,required TResult Function( CanonicalOutputValue_DateTime value)  dateTime,required TResult Function( CanonicalOutputValue_FilePath value)  filePath,required TResult Function( CanonicalOutputValue_Url value)  url,required TResult Function( CanonicalOutputValue_File value)  file,required TResult Function( CanonicalOutputValue_EmbeddedView value)  embeddedView,}){
final _that = this;
switch (_that) {
case CanonicalOutputValue_String():
return string(_that);case CanonicalOutputValue_Number():
return number(_that);case CanonicalOutputValue_Integer():
return integer(_that);case CanonicalOutputValue_Boolean():
return boolean(_that);case CanonicalOutputValue_Options():
return options(_that);case CanonicalOutputValue_MultiOptions():
return multiOptions(_that);case CanonicalOutputValue_Markdown():
return markdown(_that);case CanonicalOutputValue_Json():
return json(_that);case CanonicalOutputValue_DateTime():
return dateTime(_that);case CanonicalOutputValue_FilePath():
return filePath(_that);case CanonicalOutputValue_Url():
return url(_that);case CanonicalOutputValue_File():
return file(_that);case CanonicalOutputValue_EmbeddedView():
return embeddedView(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( CanonicalOutputValue_String value)?  string,TResult? Function( CanonicalOutputValue_Number value)?  number,TResult? Function( CanonicalOutputValue_Integer value)?  integer,TResult? Function( CanonicalOutputValue_Boolean value)?  boolean,TResult? Function( CanonicalOutputValue_Options value)?  options,TResult? Function( CanonicalOutputValue_MultiOptions value)?  multiOptions,TResult? Function( CanonicalOutputValue_Markdown value)?  markdown,TResult? Function( CanonicalOutputValue_Json value)?  json,TResult? Function( CanonicalOutputValue_DateTime value)?  dateTime,TResult? Function( CanonicalOutputValue_FilePath value)?  filePath,TResult? Function( CanonicalOutputValue_Url value)?  url,TResult? Function( CanonicalOutputValue_File value)?  file,TResult? Function( CanonicalOutputValue_EmbeddedView value)?  embeddedView,}){
final _that = this;
switch (_that) {
case CanonicalOutputValue_String() when string != null:
return string(_that);case CanonicalOutputValue_Number() when number != null:
return number(_that);case CanonicalOutputValue_Integer() when integer != null:
return integer(_that);case CanonicalOutputValue_Boolean() when boolean != null:
return boolean(_that);case CanonicalOutputValue_Options() when options != null:
return options(_that);case CanonicalOutputValue_MultiOptions() when multiOptions != null:
return multiOptions(_that);case CanonicalOutputValue_Markdown() when markdown != null:
return markdown(_that);case CanonicalOutputValue_Json() when json != null:
return json(_that);case CanonicalOutputValue_DateTime() when dateTime != null:
return dateTime(_that);case CanonicalOutputValue_FilePath() when filePath != null:
return filePath(_that);case CanonicalOutputValue_Url() when url != null:
return url(_that);case CanonicalOutputValue_File() when file != null:
return file(_that);case CanonicalOutputValue_EmbeddedView() when embeddedView != null:
return embeddedView(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String value)?  string,TResult Function( double value)?  number,TResult Function( PlatformInt64 value)?  integer,TResult Function( bool value)?  boolean,TResult Function( String value)?  options,TResult Function( List<String> value)?  multiOptions,TResult Function( String value)?  markdown,TResult Function( String value)?  json,TResult Function( String value)?  dateTime,TResult Function( String value)?  filePath,TResult Function( String value)?  url,TResult Function( CanonicalFileValue value)?  file,TResult Function( String value)?  embeddedView,required TResult orElse(),}) {final _that = this;
switch (_that) {
case CanonicalOutputValue_String() when string != null:
return string(_that.value);case CanonicalOutputValue_Number() when number != null:
return number(_that.value);case CanonicalOutputValue_Integer() when integer != null:
return integer(_that.value);case CanonicalOutputValue_Boolean() when boolean != null:
return boolean(_that.value);case CanonicalOutputValue_Options() when options != null:
return options(_that.value);case CanonicalOutputValue_MultiOptions() when multiOptions != null:
return multiOptions(_that.value);case CanonicalOutputValue_Markdown() when markdown != null:
return markdown(_that.value);case CanonicalOutputValue_Json() when json != null:
return json(_that.value);case CanonicalOutputValue_DateTime() when dateTime != null:
return dateTime(_that.value);case CanonicalOutputValue_FilePath() when filePath != null:
return filePath(_that.value);case CanonicalOutputValue_Url() when url != null:
return url(_that.value);case CanonicalOutputValue_File() when file != null:
return file(_that.value);case CanonicalOutputValue_EmbeddedView() when embeddedView != null:
return embeddedView(_that.value);case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String value)  string,required TResult Function( double value)  number,required TResult Function( PlatformInt64 value)  integer,required TResult Function( bool value)  boolean,required TResult Function( String value)  options,required TResult Function( List<String> value)  multiOptions,required TResult Function( String value)  markdown,required TResult Function( String value)  json,required TResult Function( String value)  dateTime,required TResult Function( String value)  filePath,required TResult Function( String value)  url,required TResult Function( CanonicalFileValue value)  file,required TResult Function( String value)  embeddedView,}) {final _that = this;
switch (_that) {
case CanonicalOutputValue_String():
return string(_that.value);case CanonicalOutputValue_Number():
return number(_that.value);case CanonicalOutputValue_Integer():
return integer(_that.value);case CanonicalOutputValue_Boolean():
return boolean(_that.value);case CanonicalOutputValue_Options():
return options(_that.value);case CanonicalOutputValue_MultiOptions():
return multiOptions(_that.value);case CanonicalOutputValue_Markdown():
return markdown(_that.value);case CanonicalOutputValue_Json():
return json(_that.value);case CanonicalOutputValue_DateTime():
return dateTime(_that.value);case CanonicalOutputValue_FilePath():
return filePath(_that.value);case CanonicalOutputValue_Url():
return url(_that.value);case CanonicalOutputValue_File():
return file(_that.value);case CanonicalOutputValue_EmbeddedView():
return embeddedView(_that.value);}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String value)?  string,TResult? Function( double value)?  number,TResult? Function( PlatformInt64 value)?  integer,TResult? Function( bool value)?  boolean,TResult? Function( String value)?  options,TResult? Function( List<String> value)?  multiOptions,TResult? Function( String value)?  markdown,TResult? Function( String value)?  json,TResult? Function( String value)?  dateTime,TResult? Function( String value)?  filePath,TResult? Function( String value)?  url,TResult? Function( CanonicalFileValue value)?  file,TResult? Function( String value)?  embeddedView,}) {final _that = this;
switch (_that) {
case CanonicalOutputValue_String() when string != null:
return string(_that.value);case CanonicalOutputValue_Number() when number != null:
return number(_that.value);case CanonicalOutputValue_Integer() when integer != null:
return integer(_that.value);case CanonicalOutputValue_Boolean() when boolean != null:
return boolean(_that.value);case CanonicalOutputValue_Options() when options != null:
return options(_that.value);case CanonicalOutputValue_MultiOptions() when multiOptions != null:
return multiOptions(_that.value);case CanonicalOutputValue_Markdown() when markdown != null:
return markdown(_that.value);case CanonicalOutputValue_Json() when json != null:
return json(_that.value);case CanonicalOutputValue_DateTime() when dateTime != null:
return dateTime(_that.value);case CanonicalOutputValue_FilePath() when filePath != null:
return filePath(_that.value);case CanonicalOutputValue_Url() when url != null:
return url(_that.value);case CanonicalOutputValue_File() when file != null:
return file(_that.value);case CanonicalOutputValue_EmbeddedView() when embeddedView != null:
return embeddedView(_that.value);case _:
  return null;

}
}

}

/// @nodoc


class CanonicalOutputValue_String extends CanonicalOutputValue {
  const CanonicalOutputValue_String({required this.value}): super._();


@override final  String value;

/// Create a copy of CanonicalOutputValue
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CanonicalOutputValue_StringCopyWith<CanonicalOutputValue_String> get copyWith => _$CanonicalOutputValue_StringCopyWithImpl<CanonicalOutputValue_String>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CanonicalOutputValue_String&&(identical(other.value, value) || other.value == value));
}


@override
int get hashCode => Object.hash(runtimeType,value);

@override
String toString() {
  return 'CanonicalOutputValue.string(value: $value)';
}


}

/// @nodoc
abstract mixin class $CanonicalOutputValue_StringCopyWith<$Res> implements $CanonicalOutputValueCopyWith<$Res> {
  factory $CanonicalOutputValue_StringCopyWith(CanonicalOutputValue_String value, $Res Function(CanonicalOutputValue_String) _then) = _$CanonicalOutputValue_StringCopyWithImpl;
@useResult
$Res call({
 String value
});




}
/// @nodoc
class _$CanonicalOutputValue_StringCopyWithImpl<$Res>
    implements $CanonicalOutputValue_StringCopyWith<$Res> {
  _$CanonicalOutputValue_StringCopyWithImpl(this._self, this._then);

  final CanonicalOutputValue_String _self;
  final $Res Function(CanonicalOutputValue_String) _then;

/// Create a copy of CanonicalOutputValue
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? value = null,}) {
  return _then(CanonicalOutputValue_String(
value: null == value ? _self.value : value // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class CanonicalOutputValue_Number extends CanonicalOutputValue {
  const CanonicalOutputValue_Number({required this.value}): super._();


@override final  double value;

/// Create a copy of CanonicalOutputValue
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CanonicalOutputValue_NumberCopyWith<CanonicalOutputValue_Number> get copyWith => _$CanonicalOutputValue_NumberCopyWithImpl<CanonicalOutputValue_Number>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CanonicalOutputValue_Number&&(identical(other.value, value) || other.value == value));
}


@override
int get hashCode => Object.hash(runtimeType,value);

@override
String toString() {
  return 'CanonicalOutputValue.number(value: $value)';
}


}

/// @nodoc
abstract mixin class $CanonicalOutputValue_NumberCopyWith<$Res> implements $CanonicalOutputValueCopyWith<$Res> {
  factory $CanonicalOutputValue_NumberCopyWith(CanonicalOutputValue_Number value, $Res Function(CanonicalOutputValue_Number) _then) = _$CanonicalOutputValue_NumberCopyWithImpl;
@useResult
$Res call({
 double value
});




}
/// @nodoc
class _$CanonicalOutputValue_NumberCopyWithImpl<$Res>
    implements $CanonicalOutputValue_NumberCopyWith<$Res> {
  _$CanonicalOutputValue_NumberCopyWithImpl(this._self, this._then);

  final CanonicalOutputValue_Number _self;
  final $Res Function(CanonicalOutputValue_Number) _then;

/// Create a copy of CanonicalOutputValue
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? value = null,}) {
  return _then(CanonicalOutputValue_Number(
value: null == value ? _self.value : value // ignore: cast_nullable_to_non_nullable
as double,
  ));
}


}

/// @nodoc


class CanonicalOutputValue_Integer extends CanonicalOutputValue {
  const CanonicalOutputValue_Integer({required this.value}): super._();


@override final  PlatformInt64 value;

/// Create a copy of CanonicalOutputValue
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CanonicalOutputValue_IntegerCopyWith<CanonicalOutputValue_Integer> get copyWith => _$CanonicalOutputValue_IntegerCopyWithImpl<CanonicalOutputValue_Integer>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CanonicalOutputValue_Integer&&(identical(other.value, value) || other.value == value));
}


@override
int get hashCode => Object.hash(runtimeType,value);

@override
String toString() {
  return 'CanonicalOutputValue.integer(value: $value)';
}


}

/// @nodoc
abstract mixin class $CanonicalOutputValue_IntegerCopyWith<$Res> implements $CanonicalOutputValueCopyWith<$Res> {
  factory $CanonicalOutputValue_IntegerCopyWith(CanonicalOutputValue_Integer value, $Res Function(CanonicalOutputValue_Integer) _then) = _$CanonicalOutputValue_IntegerCopyWithImpl;
@useResult
$Res call({
 PlatformInt64 value
});




}
/// @nodoc
class _$CanonicalOutputValue_IntegerCopyWithImpl<$Res>
    implements $CanonicalOutputValue_IntegerCopyWith<$Res> {
  _$CanonicalOutputValue_IntegerCopyWithImpl(this._self, this._then);

  final CanonicalOutputValue_Integer _self;
  final $Res Function(CanonicalOutputValue_Integer) _then;

/// Create a copy of CanonicalOutputValue
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? value = null,}) {
  return _then(CanonicalOutputValue_Integer(
value: null == value ? _self.value : value // ignore: cast_nullable_to_non_nullable
as PlatformInt64,
  ));
}


}

/// @nodoc


class CanonicalOutputValue_Boolean extends CanonicalOutputValue {
  const CanonicalOutputValue_Boolean({required this.value}): super._();


@override final  bool value;

/// Create a copy of CanonicalOutputValue
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CanonicalOutputValue_BooleanCopyWith<CanonicalOutputValue_Boolean> get copyWith => _$CanonicalOutputValue_BooleanCopyWithImpl<CanonicalOutputValue_Boolean>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CanonicalOutputValue_Boolean&&(identical(other.value, value) || other.value == value));
}


@override
int get hashCode => Object.hash(runtimeType,value);

@override
String toString() {
  return 'CanonicalOutputValue.boolean(value: $value)';
}


}

/// @nodoc
abstract mixin class $CanonicalOutputValue_BooleanCopyWith<$Res> implements $CanonicalOutputValueCopyWith<$Res> {
  factory $CanonicalOutputValue_BooleanCopyWith(CanonicalOutputValue_Boolean value, $Res Function(CanonicalOutputValue_Boolean) _then) = _$CanonicalOutputValue_BooleanCopyWithImpl;
@useResult
$Res call({
 bool value
});




}
/// @nodoc
class _$CanonicalOutputValue_BooleanCopyWithImpl<$Res>
    implements $CanonicalOutputValue_BooleanCopyWith<$Res> {
  _$CanonicalOutputValue_BooleanCopyWithImpl(this._self, this._then);

  final CanonicalOutputValue_Boolean _self;
  final $Res Function(CanonicalOutputValue_Boolean) _then;

/// Create a copy of CanonicalOutputValue
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? value = null,}) {
  return _then(CanonicalOutputValue_Boolean(
value: null == value ? _self.value : value // ignore: cast_nullable_to_non_nullable
as bool,
  ));
}


}

/// @nodoc


class CanonicalOutputValue_Options extends CanonicalOutputValue {
  const CanonicalOutputValue_Options({required this.value}): super._();


@override final  String value;

/// Create a copy of CanonicalOutputValue
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CanonicalOutputValue_OptionsCopyWith<CanonicalOutputValue_Options> get copyWith => _$CanonicalOutputValue_OptionsCopyWithImpl<CanonicalOutputValue_Options>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CanonicalOutputValue_Options&&(identical(other.value, value) || other.value == value));
}


@override
int get hashCode => Object.hash(runtimeType,value);

@override
String toString() {
  return 'CanonicalOutputValue.options(value: $value)';
}


}

/// @nodoc
abstract mixin class $CanonicalOutputValue_OptionsCopyWith<$Res> implements $CanonicalOutputValueCopyWith<$Res> {
  factory $CanonicalOutputValue_OptionsCopyWith(CanonicalOutputValue_Options value, $Res Function(CanonicalOutputValue_Options) _then) = _$CanonicalOutputValue_OptionsCopyWithImpl;
@useResult
$Res call({
 String value
});




}
/// @nodoc
class _$CanonicalOutputValue_OptionsCopyWithImpl<$Res>
    implements $CanonicalOutputValue_OptionsCopyWith<$Res> {
  _$CanonicalOutputValue_OptionsCopyWithImpl(this._self, this._then);

  final CanonicalOutputValue_Options _self;
  final $Res Function(CanonicalOutputValue_Options) _then;

/// Create a copy of CanonicalOutputValue
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? value = null,}) {
  return _then(CanonicalOutputValue_Options(
value: null == value ? _self.value : value // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class CanonicalOutputValue_MultiOptions extends CanonicalOutputValue {
  const CanonicalOutputValue_MultiOptions({required final  List<String> value}): _value = value,super._();


 final  List<String> _value;
@override List<String> get value {
  if (_value is EqualUnmodifiableListView) return _value;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_value);
}


/// Create a copy of CanonicalOutputValue
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CanonicalOutputValue_MultiOptionsCopyWith<CanonicalOutputValue_MultiOptions> get copyWith => _$CanonicalOutputValue_MultiOptionsCopyWithImpl<CanonicalOutputValue_MultiOptions>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CanonicalOutputValue_MultiOptions&&const DeepCollectionEquality().equals(other._value, _value));
}


@override
int get hashCode => Object.hash(runtimeType,const DeepCollectionEquality().hash(_value));

@override
String toString() {
  return 'CanonicalOutputValue.multiOptions(value: $value)';
}


}

/// @nodoc
abstract mixin class $CanonicalOutputValue_MultiOptionsCopyWith<$Res> implements $CanonicalOutputValueCopyWith<$Res> {
  factory $CanonicalOutputValue_MultiOptionsCopyWith(CanonicalOutputValue_MultiOptions value, $Res Function(CanonicalOutputValue_MultiOptions) _then) = _$CanonicalOutputValue_MultiOptionsCopyWithImpl;
@useResult
$Res call({
 List<String> value
});




}
/// @nodoc
class _$CanonicalOutputValue_MultiOptionsCopyWithImpl<$Res>
    implements $CanonicalOutputValue_MultiOptionsCopyWith<$Res> {
  _$CanonicalOutputValue_MultiOptionsCopyWithImpl(this._self, this._then);

  final CanonicalOutputValue_MultiOptions _self;
  final $Res Function(CanonicalOutputValue_MultiOptions) _then;

/// Create a copy of CanonicalOutputValue
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? value = null,}) {
  return _then(CanonicalOutputValue_MultiOptions(
value: null == value ? _self._value : value // ignore: cast_nullable_to_non_nullable
as List<String>,
  ));
}


}

/// @nodoc


class CanonicalOutputValue_Markdown extends CanonicalOutputValue {
  const CanonicalOutputValue_Markdown({required this.value}): super._();


@override final  String value;

/// Create a copy of CanonicalOutputValue
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CanonicalOutputValue_MarkdownCopyWith<CanonicalOutputValue_Markdown> get copyWith => _$CanonicalOutputValue_MarkdownCopyWithImpl<CanonicalOutputValue_Markdown>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CanonicalOutputValue_Markdown&&(identical(other.value, value) || other.value == value));
}


@override
int get hashCode => Object.hash(runtimeType,value);

@override
String toString() {
  return 'CanonicalOutputValue.markdown(value: $value)';
}


}

/// @nodoc
abstract mixin class $CanonicalOutputValue_MarkdownCopyWith<$Res> implements $CanonicalOutputValueCopyWith<$Res> {
  factory $CanonicalOutputValue_MarkdownCopyWith(CanonicalOutputValue_Markdown value, $Res Function(CanonicalOutputValue_Markdown) _then) = _$CanonicalOutputValue_MarkdownCopyWithImpl;
@useResult
$Res call({
 String value
});




}
/// @nodoc
class _$CanonicalOutputValue_MarkdownCopyWithImpl<$Res>
    implements $CanonicalOutputValue_MarkdownCopyWith<$Res> {
  _$CanonicalOutputValue_MarkdownCopyWithImpl(this._self, this._then);

  final CanonicalOutputValue_Markdown _self;
  final $Res Function(CanonicalOutputValue_Markdown) _then;

/// Create a copy of CanonicalOutputValue
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? value = null,}) {
  return _then(CanonicalOutputValue_Markdown(
value: null == value ? _self.value : value // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class CanonicalOutputValue_Json extends CanonicalOutputValue {
  const CanonicalOutputValue_Json({required this.value}): super._();


@override final  String value;

/// Create a copy of CanonicalOutputValue
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CanonicalOutputValue_JsonCopyWith<CanonicalOutputValue_Json> get copyWith => _$CanonicalOutputValue_JsonCopyWithImpl<CanonicalOutputValue_Json>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CanonicalOutputValue_Json&&(identical(other.value, value) || other.value == value));
}


@override
int get hashCode => Object.hash(runtimeType,value);

@override
String toString() {
  return 'CanonicalOutputValue.json(value: $value)';
}


}

/// @nodoc
abstract mixin class $CanonicalOutputValue_JsonCopyWith<$Res> implements $CanonicalOutputValueCopyWith<$Res> {
  factory $CanonicalOutputValue_JsonCopyWith(CanonicalOutputValue_Json value, $Res Function(CanonicalOutputValue_Json) _then) = _$CanonicalOutputValue_JsonCopyWithImpl;
@useResult
$Res call({
 String value
});




}
/// @nodoc
class _$CanonicalOutputValue_JsonCopyWithImpl<$Res>
    implements $CanonicalOutputValue_JsonCopyWith<$Res> {
  _$CanonicalOutputValue_JsonCopyWithImpl(this._self, this._then);

  final CanonicalOutputValue_Json _self;
  final $Res Function(CanonicalOutputValue_Json) _then;

/// Create a copy of CanonicalOutputValue
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? value = null,}) {
  return _then(CanonicalOutputValue_Json(
value: null == value ? _self.value : value // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class CanonicalOutputValue_DateTime extends CanonicalOutputValue {
  const CanonicalOutputValue_DateTime({required this.value}): super._();


@override final  String value;

/// Create a copy of CanonicalOutputValue
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CanonicalOutputValue_DateTimeCopyWith<CanonicalOutputValue_DateTime> get copyWith => _$CanonicalOutputValue_DateTimeCopyWithImpl<CanonicalOutputValue_DateTime>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CanonicalOutputValue_DateTime&&(identical(other.value, value) || other.value == value));
}


@override
int get hashCode => Object.hash(runtimeType,value);

@override
String toString() {
  return 'CanonicalOutputValue.dateTime(value: $value)';
}


}

/// @nodoc
abstract mixin class $CanonicalOutputValue_DateTimeCopyWith<$Res> implements $CanonicalOutputValueCopyWith<$Res> {
  factory $CanonicalOutputValue_DateTimeCopyWith(CanonicalOutputValue_DateTime value, $Res Function(CanonicalOutputValue_DateTime) _then) = _$CanonicalOutputValue_DateTimeCopyWithImpl;
@useResult
$Res call({
 String value
});




}
/// @nodoc
class _$CanonicalOutputValue_DateTimeCopyWithImpl<$Res>
    implements $CanonicalOutputValue_DateTimeCopyWith<$Res> {
  _$CanonicalOutputValue_DateTimeCopyWithImpl(this._self, this._then);

  final CanonicalOutputValue_DateTime _self;
  final $Res Function(CanonicalOutputValue_DateTime) _then;

/// Create a copy of CanonicalOutputValue
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? value = null,}) {
  return _then(CanonicalOutputValue_DateTime(
value: null == value ? _self.value : value // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class CanonicalOutputValue_FilePath extends CanonicalOutputValue {
  const CanonicalOutputValue_FilePath({required this.value}): super._();


@override final  String value;

/// Create a copy of CanonicalOutputValue
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CanonicalOutputValue_FilePathCopyWith<CanonicalOutputValue_FilePath> get copyWith => _$CanonicalOutputValue_FilePathCopyWithImpl<CanonicalOutputValue_FilePath>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CanonicalOutputValue_FilePath&&(identical(other.value, value) || other.value == value));
}


@override
int get hashCode => Object.hash(runtimeType,value);

@override
String toString() {
  return 'CanonicalOutputValue.filePath(value: $value)';
}


}

/// @nodoc
abstract mixin class $CanonicalOutputValue_FilePathCopyWith<$Res> implements $CanonicalOutputValueCopyWith<$Res> {
  factory $CanonicalOutputValue_FilePathCopyWith(CanonicalOutputValue_FilePath value, $Res Function(CanonicalOutputValue_FilePath) _then) = _$CanonicalOutputValue_FilePathCopyWithImpl;
@useResult
$Res call({
 String value
});




}
/// @nodoc
class _$CanonicalOutputValue_FilePathCopyWithImpl<$Res>
    implements $CanonicalOutputValue_FilePathCopyWith<$Res> {
  _$CanonicalOutputValue_FilePathCopyWithImpl(this._self, this._then);

  final CanonicalOutputValue_FilePath _self;
  final $Res Function(CanonicalOutputValue_FilePath) _then;

/// Create a copy of CanonicalOutputValue
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? value = null,}) {
  return _then(CanonicalOutputValue_FilePath(
value: null == value ? _self.value : value // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class CanonicalOutputValue_Url extends CanonicalOutputValue {
  const CanonicalOutputValue_Url({required this.value}): super._();


@override final  String value;

/// Create a copy of CanonicalOutputValue
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CanonicalOutputValue_UrlCopyWith<CanonicalOutputValue_Url> get copyWith => _$CanonicalOutputValue_UrlCopyWithImpl<CanonicalOutputValue_Url>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CanonicalOutputValue_Url&&(identical(other.value, value) || other.value == value));
}


@override
int get hashCode => Object.hash(runtimeType,value);

@override
String toString() {
  return 'CanonicalOutputValue.url(value: $value)';
}


}

/// @nodoc
abstract mixin class $CanonicalOutputValue_UrlCopyWith<$Res> implements $CanonicalOutputValueCopyWith<$Res> {
  factory $CanonicalOutputValue_UrlCopyWith(CanonicalOutputValue_Url value, $Res Function(CanonicalOutputValue_Url) _then) = _$CanonicalOutputValue_UrlCopyWithImpl;
@useResult
$Res call({
 String value
});




}
/// @nodoc
class _$CanonicalOutputValue_UrlCopyWithImpl<$Res>
    implements $CanonicalOutputValue_UrlCopyWith<$Res> {
  _$CanonicalOutputValue_UrlCopyWithImpl(this._self, this._then);

  final CanonicalOutputValue_Url _self;
  final $Res Function(CanonicalOutputValue_Url) _then;

/// Create a copy of CanonicalOutputValue
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? value = null,}) {
  return _then(CanonicalOutputValue_Url(
value: null == value ? _self.value : value // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class CanonicalOutputValue_File extends CanonicalOutputValue {
  const CanonicalOutputValue_File({required this.value}): super._();


@override final  CanonicalFileValue value;

/// Create a copy of CanonicalOutputValue
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CanonicalOutputValue_FileCopyWith<CanonicalOutputValue_File> get copyWith => _$CanonicalOutputValue_FileCopyWithImpl<CanonicalOutputValue_File>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CanonicalOutputValue_File&&(identical(other.value, value) || other.value == value));
}


@override
int get hashCode => Object.hash(runtimeType,value);

@override
String toString() {
  return 'CanonicalOutputValue.file(value: $value)';
}


}

/// @nodoc
abstract mixin class $CanonicalOutputValue_FileCopyWith<$Res> implements $CanonicalOutputValueCopyWith<$Res> {
  factory $CanonicalOutputValue_FileCopyWith(CanonicalOutputValue_File value, $Res Function(CanonicalOutputValue_File) _then) = _$CanonicalOutputValue_FileCopyWithImpl;
@useResult
$Res call({
 CanonicalFileValue value
});




}
/// @nodoc
class _$CanonicalOutputValue_FileCopyWithImpl<$Res>
    implements $CanonicalOutputValue_FileCopyWith<$Res> {
  _$CanonicalOutputValue_FileCopyWithImpl(this._self, this._then);

  final CanonicalOutputValue_File _self;
  final $Res Function(CanonicalOutputValue_File) _then;

/// Create a copy of CanonicalOutputValue
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? value = null,}) {
  return _then(CanonicalOutputValue_File(
value: null == value ? _self.value : value // ignore: cast_nullable_to_non_nullable
as CanonicalFileValue,
  ));
}


}

/// @nodoc


class CanonicalOutputValue_EmbeddedView extends CanonicalOutputValue {
  const CanonicalOutputValue_EmbeddedView({required this.value}): super._();


@override final  String value;

/// Create a copy of CanonicalOutputValue
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CanonicalOutputValue_EmbeddedViewCopyWith<CanonicalOutputValue_EmbeddedView> get copyWith => _$CanonicalOutputValue_EmbeddedViewCopyWithImpl<CanonicalOutputValue_EmbeddedView>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CanonicalOutputValue_EmbeddedView&&(identical(other.value, value) || other.value == value));
}


@override
int get hashCode => Object.hash(runtimeType,value);

@override
String toString() {
  return 'CanonicalOutputValue.embeddedView(value: $value)';
}


}

/// @nodoc
abstract mixin class $CanonicalOutputValue_EmbeddedViewCopyWith<$Res> implements $CanonicalOutputValueCopyWith<$Res> {
  factory $CanonicalOutputValue_EmbeddedViewCopyWith(CanonicalOutputValue_EmbeddedView value, $Res Function(CanonicalOutputValue_EmbeddedView) _then) = _$CanonicalOutputValue_EmbeddedViewCopyWithImpl;
@useResult
$Res call({
 String value
});




}
/// @nodoc
class _$CanonicalOutputValue_EmbeddedViewCopyWithImpl<$Res>
    implements $CanonicalOutputValue_EmbeddedViewCopyWith<$Res> {
  _$CanonicalOutputValue_EmbeddedViewCopyWithImpl(this._self, this._then);

  final CanonicalOutputValue_EmbeddedView _self;
  final $Res Function(CanonicalOutputValue_EmbeddedView) _then;

/// Create a copy of CanonicalOutputValue
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? value = null,}) {
  return _then(CanonicalOutputValue_EmbeddedView(
value: null == value ? _self.value : value // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc
mixin _$OutputFieldType {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is OutputFieldType);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'OutputFieldType()';
}


}

/// @nodoc
class $OutputFieldTypeCopyWith<$Res>  {
$OutputFieldTypeCopyWith(OutputFieldType _, $Res Function(OutputFieldType) __);
}


/// Adds pattern-matching-related methods to [OutputFieldType].
extension OutputFieldTypePatterns on OutputFieldType {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( OutputFieldType_Text value)?  text,TResult Function( OutputFieldType_Number value)?  number,TResult Function( OutputFieldType_Integer value)?  integer,TResult Function( OutputFieldType_Boolean value)?  boolean,TResult Function( OutputFieldType_File value)?  file,TResult Function( OutputFieldType_Select value)?  select,TResult Function( OutputFieldType_Multiline value)?  multiline,TResult Function( OutputFieldType_MultiOptions value)?  multiOptions,TResult Function( OutputFieldType_Json value)?  json,TResult Function( OutputFieldType_DateTime value)?  dateTime,TResult Function( OutputFieldType_Markdown value)?  markdown,TResult Function( OutputFieldType_FilePath value)?  filePath,TResult Function( OutputFieldType_Url value)?  url,TResult Function( OutputFieldType_EmbeddedView value)?  embeddedView,required TResult orElse(),}){
final _that = this;
switch (_that) {
case OutputFieldType_Text() when text != null:
return text(_that);case OutputFieldType_Number() when number != null:
return number(_that);case OutputFieldType_Integer() when integer != null:
return integer(_that);case OutputFieldType_Boolean() when boolean != null:
return boolean(_that);case OutputFieldType_File() when file != null:
return file(_that);case OutputFieldType_Select() when select != null:
return select(_that);case OutputFieldType_Multiline() when multiline != null:
return multiline(_that);case OutputFieldType_MultiOptions() when multiOptions != null:
return multiOptions(_that);case OutputFieldType_Json() when json != null:
return json(_that);case OutputFieldType_DateTime() when dateTime != null:
return dateTime(_that);case OutputFieldType_Markdown() when markdown != null:
return markdown(_that);case OutputFieldType_FilePath() when filePath != null:
return filePath(_that);case OutputFieldType_Url() when url != null:
return url(_that);case OutputFieldType_EmbeddedView() when embeddedView != null:
return embeddedView(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( OutputFieldType_Text value)  text,required TResult Function( OutputFieldType_Number value)  number,required TResult Function( OutputFieldType_Integer value)  integer,required TResult Function( OutputFieldType_Boolean value)  boolean,required TResult Function( OutputFieldType_File value)  file,required TResult Function( OutputFieldType_Select value)  select,required TResult Function( OutputFieldType_Multiline value)  multiline,required TResult Function( OutputFieldType_MultiOptions value)  multiOptions,required TResult Function( OutputFieldType_Json value)  json,required TResult Function( OutputFieldType_DateTime value)  dateTime,required TResult Function( OutputFieldType_Markdown value)  markdown,required TResult Function( OutputFieldType_FilePath value)  filePath,required TResult Function( OutputFieldType_Url value)  url,required TResult Function( OutputFieldType_EmbeddedView value)  embeddedView,}){
final _that = this;
switch (_that) {
case OutputFieldType_Text():
return text(_that);case OutputFieldType_Number():
return number(_that);case OutputFieldType_Integer():
return integer(_that);case OutputFieldType_Boolean():
return boolean(_that);case OutputFieldType_File():
return file(_that);case OutputFieldType_Select():
return select(_that);case OutputFieldType_Multiline():
return multiline(_that);case OutputFieldType_MultiOptions():
return multiOptions(_that);case OutputFieldType_Json():
return json(_that);case OutputFieldType_DateTime():
return dateTime(_that);case OutputFieldType_Markdown():
return markdown(_that);case OutputFieldType_FilePath():
return filePath(_that);case OutputFieldType_Url():
return url(_that);case OutputFieldType_EmbeddedView():
return embeddedView(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( OutputFieldType_Text value)?  text,TResult? Function( OutputFieldType_Number value)?  number,TResult? Function( OutputFieldType_Integer value)?  integer,TResult? Function( OutputFieldType_Boolean value)?  boolean,TResult? Function( OutputFieldType_File value)?  file,TResult? Function( OutputFieldType_Select value)?  select,TResult? Function( OutputFieldType_Multiline value)?  multiline,TResult? Function( OutputFieldType_MultiOptions value)?  multiOptions,TResult? Function( OutputFieldType_Json value)?  json,TResult? Function( OutputFieldType_DateTime value)?  dateTime,TResult? Function( OutputFieldType_Markdown value)?  markdown,TResult? Function( OutputFieldType_FilePath value)?  filePath,TResult? Function( OutputFieldType_Url value)?  url,TResult? Function( OutputFieldType_EmbeddedView value)?  embeddedView,}){
final _that = this;
switch (_that) {
case OutputFieldType_Text() when text != null:
return text(_that);case OutputFieldType_Number() when number != null:
return number(_that);case OutputFieldType_Integer() when integer != null:
return integer(_that);case OutputFieldType_Boolean() when boolean != null:
return boolean(_that);case OutputFieldType_File() when file != null:
return file(_that);case OutputFieldType_Select() when select != null:
return select(_that);case OutputFieldType_Multiline() when multiline != null:
return multiline(_that);case OutputFieldType_MultiOptions() when multiOptions != null:
return multiOptions(_that);case OutputFieldType_Json() when json != null:
return json(_that);case OutputFieldType_DateTime() when dateTime != null:
return dateTime(_that);case OutputFieldType_Markdown() when markdown != null:
return markdown(_that);case OutputFieldType_FilePath() when filePath != null:
return filePath(_that);case OutputFieldType_Url() when url != null:
return url(_that);case OutputFieldType_EmbeddedView() when embeddedView != null:
return embeddedView(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function()?  text,TResult Function()?  number,TResult Function()?  integer,TResult Function()?  boolean,TResult Function()?  file,TResult Function( List<String> options)?  select,TResult Function()?  multiline,TResult Function( List<String> options)?  multiOptions,TResult Function()?  json,TResult Function()?  dateTime,TResult Function()?  markdown,TResult Function()?  filePath,TResult Function()?  url,TResult Function( String url)?  embeddedView,required TResult orElse(),}) {final _that = this;
switch (_that) {
case OutputFieldType_Text() when text != null:
return text();case OutputFieldType_Number() when number != null:
return number();case OutputFieldType_Integer() when integer != null:
return integer();case OutputFieldType_Boolean() when boolean != null:
return boolean();case OutputFieldType_File() when file != null:
return file();case OutputFieldType_Select() when select != null:
return select(_that.options);case OutputFieldType_Multiline() when multiline != null:
return multiline();case OutputFieldType_MultiOptions() when multiOptions != null:
return multiOptions(_that.options);case OutputFieldType_Json() when json != null:
return json();case OutputFieldType_DateTime() when dateTime != null:
return dateTime();case OutputFieldType_Markdown() when markdown != null:
return markdown();case OutputFieldType_FilePath() when filePath != null:
return filePath();case OutputFieldType_Url() when url != null:
return url();case OutputFieldType_EmbeddedView() when embeddedView != null:
return embeddedView(_that.url);case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function()  text,required TResult Function()  number,required TResult Function()  integer,required TResult Function()  boolean,required TResult Function()  file,required TResult Function( List<String> options)  select,required TResult Function()  multiline,required TResult Function( List<String> options)  multiOptions,required TResult Function()  json,required TResult Function()  dateTime,required TResult Function()  markdown,required TResult Function()  filePath,required TResult Function()  url,required TResult Function( String url)  embeddedView,}) {final _that = this;
switch (_that) {
case OutputFieldType_Text():
return text();case OutputFieldType_Number():
return number();case OutputFieldType_Integer():
return integer();case OutputFieldType_Boolean():
return boolean();case OutputFieldType_File():
return file();case OutputFieldType_Select():
return select(_that.options);case OutputFieldType_Multiline():
return multiline();case OutputFieldType_MultiOptions():
return multiOptions(_that.options);case OutputFieldType_Json():
return json();case OutputFieldType_DateTime():
return dateTime();case OutputFieldType_Markdown():
return markdown();case OutputFieldType_FilePath():
return filePath();case OutputFieldType_Url():
return url();case OutputFieldType_EmbeddedView():
return embeddedView(_that.url);}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function()?  text,TResult? Function()?  number,TResult? Function()?  integer,TResult? Function()?  boolean,TResult? Function()?  file,TResult? Function( List<String> options)?  select,TResult? Function()?  multiline,TResult? Function( List<String> options)?  multiOptions,TResult? Function()?  json,TResult? Function()?  dateTime,TResult? Function()?  markdown,TResult? Function()?  filePath,TResult? Function()?  url,TResult? Function( String url)?  embeddedView,}) {final _that = this;
switch (_that) {
case OutputFieldType_Text() when text != null:
return text();case OutputFieldType_Number() when number != null:
return number();case OutputFieldType_Integer() when integer != null:
return integer();case OutputFieldType_Boolean() when boolean != null:
return boolean();case OutputFieldType_File() when file != null:
return file();case OutputFieldType_Select() when select != null:
return select(_that.options);case OutputFieldType_Multiline() when multiline != null:
return multiline();case OutputFieldType_MultiOptions() when multiOptions != null:
return multiOptions(_that.options);case OutputFieldType_Json() when json != null:
return json();case OutputFieldType_DateTime() when dateTime != null:
return dateTime();case OutputFieldType_Markdown() when markdown != null:
return markdown();case OutputFieldType_FilePath() when filePath != null:
return filePath();case OutputFieldType_Url() when url != null:
return url();case OutputFieldType_EmbeddedView() when embeddedView != null:
return embeddedView(_that.url);case _:
  return null;

}
}

}

/// @nodoc


class OutputFieldType_Text extends OutputFieldType {
  const OutputFieldType_Text(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is OutputFieldType_Text);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'OutputFieldType.text()';
}


}




/// @nodoc


class OutputFieldType_Number extends OutputFieldType {
  const OutputFieldType_Number(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is OutputFieldType_Number);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'OutputFieldType.number()';
}


}




/// @nodoc


class OutputFieldType_Integer extends OutputFieldType {
  const OutputFieldType_Integer(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is OutputFieldType_Integer);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'OutputFieldType.integer()';
}


}




/// @nodoc


class OutputFieldType_Boolean extends OutputFieldType {
  const OutputFieldType_Boolean(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is OutputFieldType_Boolean);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'OutputFieldType.boolean()';
}


}




/// @nodoc


class OutputFieldType_File extends OutputFieldType {
  const OutputFieldType_File(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is OutputFieldType_File);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'OutputFieldType.file()';
}


}




/// @nodoc


class OutputFieldType_Select extends OutputFieldType {
  const OutputFieldType_Select({required final  List<String> options}): _options = options,super._();


 final  List<String> _options;
 List<String> get options {
  if (_options is EqualUnmodifiableListView) return _options;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_options);
}


/// Create a copy of OutputFieldType
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$OutputFieldType_SelectCopyWith<OutputFieldType_Select> get copyWith => _$OutputFieldType_SelectCopyWithImpl<OutputFieldType_Select>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is OutputFieldType_Select&&const DeepCollectionEquality().equals(other._options, _options));
}


@override
int get hashCode => Object.hash(runtimeType,const DeepCollectionEquality().hash(_options));

@override
String toString() {
  return 'OutputFieldType.select(options: $options)';
}


}

/// @nodoc
abstract mixin class $OutputFieldType_SelectCopyWith<$Res> implements $OutputFieldTypeCopyWith<$Res> {
  factory $OutputFieldType_SelectCopyWith(OutputFieldType_Select value, $Res Function(OutputFieldType_Select) _then) = _$OutputFieldType_SelectCopyWithImpl;
@useResult
$Res call({
 List<String> options
});




}
/// @nodoc
class _$OutputFieldType_SelectCopyWithImpl<$Res>
    implements $OutputFieldType_SelectCopyWith<$Res> {
  _$OutputFieldType_SelectCopyWithImpl(this._self, this._then);

  final OutputFieldType_Select _self;
  final $Res Function(OutputFieldType_Select) _then;

/// Create a copy of OutputFieldType
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? options = null,}) {
  return _then(OutputFieldType_Select(
options: null == options ? _self._options : options // ignore: cast_nullable_to_non_nullable
as List<String>,
  ));
}


}

/// @nodoc


class OutputFieldType_Multiline extends OutputFieldType {
  const OutputFieldType_Multiline(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is OutputFieldType_Multiline);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'OutputFieldType.multiline()';
}


}




/// @nodoc


class OutputFieldType_MultiOptions extends OutputFieldType {
  const OutputFieldType_MultiOptions({required final  List<String> options}): _options = options,super._();


 final  List<String> _options;
 List<String> get options {
  if (_options is EqualUnmodifiableListView) return _options;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_options);
}


/// Create a copy of OutputFieldType
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$OutputFieldType_MultiOptionsCopyWith<OutputFieldType_MultiOptions> get copyWith => _$OutputFieldType_MultiOptionsCopyWithImpl<OutputFieldType_MultiOptions>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is OutputFieldType_MultiOptions&&const DeepCollectionEquality().equals(other._options, _options));
}


@override
int get hashCode => Object.hash(runtimeType,const DeepCollectionEquality().hash(_options));

@override
String toString() {
  return 'OutputFieldType.multiOptions(options: $options)';
}


}

/// @nodoc
abstract mixin class $OutputFieldType_MultiOptionsCopyWith<$Res> implements $OutputFieldTypeCopyWith<$Res> {
  factory $OutputFieldType_MultiOptionsCopyWith(OutputFieldType_MultiOptions value, $Res Function(OutputFieldType_MultiOptions) _then) = _$OutputFieldType_MultiOptionsCopyWithImpl;
@useResult
$Res call({
 List<String> options
});




}
/// @nodoc
class _$OutputFieldType_MultiOptionsCopyWithImpl<$Res>
    implements $OutputFieldType_MultiOptionsCopyWith<$Res> {
  _$OutputFieldType_MultiOptionsCopyWithImpl(this._self, this._then);

  final OutputFieldType_MultiOptions _self;
  final $Res Function(OutputFieldType_MultiOptions) _then;

/// Create a copy of OutputFieldType
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? options = null,}) {
  return _then(OutputFieldType_MultiOptions(
options: null == options ? _self._options : options // ignore: cast_nullable_to_non_nullable
as List<String>,
  ));
}


}

/// @nodoc


class OutputFieldType_Json extends OutputFieldType {
  const OutputFieldType_Json(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is OutputFieldType_Json);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'OutputFieldType.json()';
}


}




/// @nodoc


class OutputFieldType_DateTime extends OutputFieldType {
  const OutputFieldType_DateTime(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is OutputFieldType_DateTime);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'OutputFieldType.dateTime()';
}


}




/// @nodoc


class OutputFieldType_Markdown extends OutputFieldType {
  const OutputFieldType_Markdown(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is OutputFieldType_Markdown);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'OutputFieldType.markdown()';
}


}




/// @nodoc


class OutputFieldType_FilePath extends OutputFieldType {
  const OutputFieldType_FilePath(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is OutputFieldType_FilePath);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'OutputFieldType.filePath()';
}


}




/// @nodoc


class OutputFieldType_Url extends OutputFieldType {
  const OutputFieldType_Url(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is OutputFieldType_Url);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'OutputFieldType.url()';
}


}




/// @nodoc


class OutputFieldType_EmbeddedView extends OutputFieldType {
  const OutputFieldType_EmbeddedView({required this.url}): super._();


 final  String url;

/// Create a copy of OutputFieldType
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$OutputFieldType_EmbeddedViewCopyWith<OutputFieldType_EmbeddedView> get copyWith => _$OutputFieldType_EmbeddedViewCopyWithImpl<OutputFieldType_EmbeddedView>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is OutputFieldType_EmbeddedView&&(identical(other.url, url) || other.url == url));
}


@override
int get hashCode => Object.hash(runtimeType,url);

@override
String toString() {
  return 'OutputFieldType.embeddedView(url: $url)';
}


}

/// @nodoc
abstract mixin class $OutputFieldType_EmbeddedViewCopyWith<$Res> implements $OutputFieldTypeCopyWith<$Res> {
  factory $OutputFieldType_EmbeddedViewCopyWith(OutputFieldType_EmbeddedView value, $Res Function(OutputFieldType_EmbeddedView) _then) = _$OutputFieldType_EmbeddedViewCopyWithImpl;
@useResult
$Res call({
 String url
});




}
/// @nodoc
class _$OutputFieldType_EmbeddedViewCopyWithImpl<$Res>
    implements $OutputFieldType_EmbeddedViewCopyWith<$Res> {
  _$OutputFieldType_EmbeddedViewCopyWithImpl(this._self, this._then);

  final OutputFieldType_EmbeddedView _self;
  final $Res Function(OutputFieldType_EmbeddedView) _then;

/// Create a copy of OutputFieldType
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? url = null,}) {
  return _then(OutputFieldType_EmbeddedView(
url: null == url ? _self.url : url // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc
mixin _$SourceDto {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SourceDto);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'SourceDto()';
}


}

/// @nodoc
class $SourceDtoCopyWith<$Res>  {
$SourceDtoCopyWith(SourceDto _, $Res Function(SourceDto) __);
}


/// Adds pattern-matching-related methods to [SourceDto].
extension SourceDtoPatterns on SourceDto {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( SourceDto_UserInput value)?  userInput,TResult Function( SourceDto_Timer value)?  timer,TResult Function( SourceDto_Shortcut value)?  shortcut,TResult Function( SourceDto_Manual value)?  manual,TResult Function( SourceDto_Static value)?  static_,required TResult orElse(),}){
final _that = this;
switch (_that) {
case SourceDto_UserInput() when userInput != null:
return userInput(_that);case SourceDto_Timer() when timer != null:
return timer(_that);case SourceDto_Shortcut() when shortcut != null:
return shortcut(_that);case SourceDto_Manual() when manual != null:
return manual(_that);case SourceDto_Static() when static_ != null:
return static_(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( SourceDto_UserInput value)  userInput,required TResult Function( SourceDto_Timer value)  timer,required TResult Function( SourceDto_Shortcut value)  shortcut,required TResult Function( SourceDto_Manual value)  manual,required TResult Function( SourceDto_Static value)  static_,}){
final _that = this;
switch (_that) {
case SourceDto_UserInput():
return userInput(_that);case SourceDto_Timer():
return timer(_that);case SourceDto_Shortcut():
return shortcut(_that);case SourceDto_Manual():
return manual(_that);case SourceDto_Static():
return static_(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( SourceDto_UserInput value)?  userInput,TResult? Function( SourceDto_Timer value)?  timer,TResult? Function( SourceDto_Shortcut value)?  shortcut,TResult? Function( SourceDto_Manual value)?  manual,TResult? Function( SourceDto_Static value)?  static_,}){
final _that = this;
switch (_that) {
case SourceDto_UserInput() when userInput != null:
return userInput(_that);case SourceDto_Timer() when timer != null:
return timer(_that);case SourceDto_Shortcut() when shortcut != null:
return shortcut(_that);case SourceDto_Manual() when manual != null:
return manual(_that);case SourceDto_Static() when static_ != null:
return static_(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function()?  userInput,TResult Function( BigInt intervalMs)?  timer,TResult Function( String keys)?  shortcut,TResult Function()?  manual,TResult Function()?  static_,required TResult orElse(),}) {final _that = this;
switch (_that) {
case SourceDto_UserInput() when userInput != null:
return userInput();case SourceDto_Timer() when timer != null:
return timer(_that.intervalMs);case SourceDto_Shortcut() when shortcut != null:
return shortcut(_that.keys);case SourceDto_Manual() when manual != null:
return manual();case SourceDto_Static() when static_ != null:
return static_();case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function()  userInput,required TResult Function( BigInt intervalMs)  timer,required TResult Function( String keys)  shortcut,required TResult Function()  manual,required TResult Function()  static_,}) {final _that = this;
switch (_that) {
case SourceDto_UserInput():
return userInput();case SourceDto_Timer():
return timer(_that.intervalMs);case SourceDto_Shortcut():
return shortcut(_that.keys);case SourceDto_Manual():
return manual();case SourceDto_Static():
return static_();}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function()?  userInput,TResult? Function( BigInt intervalMs)?  timer,TResult? Function( String keys)?  shortcut,TResult? Function()?  manual,TResult? Function()?  static_,}) {final _that = this;
switch (_that) {
case SourceDto_UserInput() when userInput != null:
return userInput();case SourceDto_Timer() when timer != null:
return timer(_that.intervalMs);case SourceDto_Shortcut() when shortcut != null:
return shortcut(_that.keys);case SourceDto_Manual() when manual != null:
return manual();case SourceDto_Static() when static_ != null:
return static_();case _:
  return null;

}
}

}

/// @nodoc


class SourceDto_UserInput extends SourceDto {
  const SourceDto_UserInput(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SourceDto_UserInput);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'SourceDto.userInput()';
}


}




/// @nodoc


class SourceDto_Timer extends SourceDto {
  const SourceDto_Timer({required this.intervalMs}): super._();


 final  BigInt intervalMs;

/// Create a copy of SourceDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SourceDto_TimerCopyWith<SourceDto_Timer> get copyWith => _$SourceDto_TimerCopyWithImpl<SourceDto_Timer>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SourceDto_Timer&&(identical(other.intervalMs, intervalMs) || other.intervalMs == intervalMs));
}


@override
int get hashCode => Object.hash(runtimeType,intervalMs);

@override
String toString() {
  return 'SourceDto.timer(intervalMs: $intervalMs)';
}


}

/// @nodoc
abstract mixin class $SourceDto_TimerCopyWith<$Res> implements $SourceDtoCopyWith<$Res> {
  factory $SourceDto_TimerCopyWith(SourceDto_Timer value, $Res Function(SourceDto_Timer) _then) = _$SourceDto_TimerCopyWithImpl;
@useResult
$Res call({
 BigInt intervalMs
});




}
/// @nodoc
class _$SourceDto_TimerCopyWithImpl<$Res>
    implements $SourceDto_TimerCopyWith<$Res> {
  _$SourceDto_TimerCopyWithImpl(this._self, this._then);

  final SourceDto_Timer _self;
  final $Res Function(SourceDto_Timer) _then;

/// Create a copy of SourceDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? intervalMs = null,}) {
  return _then(SourceDto_Timer(
intervalMs: null == intervalMs ? _self.intervalMs : intervalMs // ignore: cast_nullable_to_non_nullable
as BigInt,
  ));
}


}

/// @nodoc


class SourceDto_Shortcut extends SourceDto {
  const SourceDto_Shortcut({required this.keys}): super._();


 final  String keys;

/// Create a copy of SourceDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SourceDto_ShortcutCopyWith<SourceDto_Shortcut> get copyWith => _$SourceDto_ShortcutCopyWithImpl<SourceDto_Shortcut>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SourceDto_Shortcut&&(identical(other.keys, keys) || other.keys == keys));
}


@override
int get hashCode => Object.hash(runtimeType,keys);

@override
String toString() {
  return 'SourceDto.shortcut(keys: $keys)';
}


}

/// @nodoc
abstract mixin class $SourceDto_ShortcutCopyWith<$Res> implements $SourceDtoCopyWith<$Res> {
  factory $SourceDto_ShortcutCopyWith(SourceDto_Shortcut value, $Res Function(SourceDto_Shortcut) _then) = _$SourceDto_ShortcutCopyWithImpl;
@useResult
$Res call({
 String keys
});




}
/// @nodoc
class _$SourceDto_ShortcutCopyWithImpl<$Res>
    implements $SourceDto_ShortcutCopyWith<$Res> {
  _$SourceDto_ShortcutCopyWithImpl(this._self, this._then);

  final SourceDto_Shortcut _self;
  final $Res Function(SourceDto_Shortcut) _then;

/// Create a copy of SourceDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? keys = null,}) {
  return _then(SourceDto_Shortcut(
keys: null == keys ? _self.keys : keys // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class SourceDto_Manual extends SourceDto {
  const SourceDto_Manual(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SourceDto_Manual);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'SourceDto.manual()';
}


}




/// @nodoc


class SourceDto_Static extends SourceDto {
  const SourceDto_Static(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SourceDto_Static);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'SourceDto.static_()';
}


}




// dart format on
