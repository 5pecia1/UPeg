// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'input_field.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// dart format off
T _$identity<T>(T value) => value;
/// @nodoc
mixin _$InputFieldType {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is InputFieldType);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'InputFieldType()';
}


}

/// @nodoc
class $InputFieldTypeCopyWith<$Res>  {
$InputFieldTypeCopyWith(InputFieldType _, $Res Function(InputFieldType) __);
}


/// Adds pattern-matching-related methods to [InputFieldType].
extension InputFieldTypePatterns on InputFieldType {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( InputFieldType_Text value)?  text,TResult Function( InputFieldType_Number value)?  number,TResult Function( InputFieldType_Integer value)?  integer,TResult Function( InputFieldType_Boolean value)?  boolean,TResult Function( InputFieldType_File value)?  file,TResult Function( InputFieldType_Select value)?  select,TResult Function( InputFieldType_Multiline value)?  multiline,TResult Function( InputFieldType_MultiOptions value)?  multiOptions,TResult Function( InputFieldType_DateTime value)?  dateTime,TResult Function( InputFieldType_Markdown value)?  markdown,TResult Function( InputFieldType_FilePath value)?  filePath,TResult Function( InputFieldType_Url value)?  url,required TResult orElse(),}){
final _that = this;
switch (_that) {
case InputFieldType_Text() when text != null:
return text(_that);case InputFieldType_Number() when number != null:
return number(_that);case InputFieldType_Integer() when integer != null:
return integer(_that);case InputFieldType_Boolean() when boolean != null:
return boolean(_that);case InputFieldType_File() when file != null:
return file(_that);case InputFieldType_Select() when select != null:
return select(_that);case InputFieldType_Multiline() when multiline != null:
return multiline(_that);case InputFieldType_MultiOptions() when multiOptions != null:
return multiOptions(_that);case InputFieldType_DateTime() when dateTime != null:
return dateTime(_that);case InputFieldType_Markdown() when markdown != null:
return markdown(_that);case InputFieldType_FilePath() when filePath != null:
return filePath(_that);case InputFieldType_Url() when url != null:
return url(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( InputFieldType_Text value)  text,required TResult Function( InputFieldType_Number value)  number,required TResult Function( InputFieldType_Integer value)  integer,required TResult Function( InputFieldType_Boolean value)  boolean,required TResult Function( InputFieldType_File value)  file,required TResult Function( InputFieldType_Select value)  select,required TResult Function( InputFieldType_Multiline value)  multiline,required TResult Function( InputFieldType_MultiOptions value)  multiOptions,required TResult Function( InputFieldType_DateTime value)  dateTime,required TResult Function( InputFieldType_Markdown value)  markdown,required TResult Function( InputFieldType_FilePath value)  filePath,required TResult Function( InputFieldType_Url value)  url,}){
final _that = this;
switch (_that) {
case InputFieldType_Text():
return text(_that);case InputFieldType_Number():
return number(_that);case InputFieldType_Integer():
return integer(_that);case InputFieldType_Boolean():
return boolean(_that);case InputFieldType_File():
return file(_that);case InputFieldType_Select():
return select(_that);case InputFieldType_Multiline():
return multiline(_that);case InputFieldType_MultiOptions():
return multiOptions(_that);case InputFieldType_DateTime():
return dateTime(_that);case InputFieldType_Markdown():
return markdown(_that);case InputFieldType_FilePath():
return filePath(_that);case InputFieldType_Url():
return url(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( InputFieldType_Text value)?  text,TResult? Function( InputFieldType_Number value)?  number,TResult? Function( InputFieldType_Integer value)?  integer,TResult? Function( InputFieldType_Boolean value)?  boolean,TResult? Function( InputFieldType_File value)?  file,TResult? Function( InputFieldType_Select value)?  select,TResult? Function( InputFieldType_Multiline value)?  multiline,TResult? Function( InputFieldType_MultiOptions value)?  multiOptions,TResult? Function( InputFieldType_DateTime value)?  dateTime,TResult? Function( InputFieldType_Markdown value)?  markdown,TResult? Function( InputFieldType_FilePath value)?  filePath,TResult? Function( InputFieldType_Url value)?  url,}){
final _that = this;
switch (_that) {
case InputFieldType_Text() when text != null:
return text(_that);case InputFieldType_Number() when number != null:
return number(_that);case InputFieldType_Integer() when integer != null:
return integer(_that);case InputFieldType_Boolean() when boolean != null:
return boolean(_that);case InputFieldType_File() when file != null:
return file(_that);case InputFieldType_Select() when select != null:
return select(_that);case InputFieldType_Multiline() when multiline != null:
return multiline(_that);case InputFieldType_MultiOptions() when multiOptions != null:
return multiOptions(_that);case InputFieldType_DateTime() when dateTime != null:
return dateTime(_that);case InputFieldType_Markdown() when markdown != null:
return markdown(_that);case InputFieldType_FilePath() when filePath != null:
return filePath(_that);case InputFieldType_Url() when url != null:
return url(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function()?  text,TResult Function()?  number,TResult Function()?  integer,TResult Function()?  boolean,TResult Function( FileInputPolicyDto policy)?  file,TResult Function( List<ChoiceOptionDto> options)?  select,TResult Function()?  multiline,TResult Function( List<ChoiceOptionDto> options)?  multiOptions,TResult Function()?  dateTime,TResult Function()?  markdown,TResult Function()?  filePath,TResult Function()?  url,required TResult orElse(),}) {final _that = this;
switch (_that) {
case InputFieldType_Text() when text != null:
return text();case InputFieldType_Number() when number != null:
return number();case InputFieldType_Integer() when integer != null:
return integer();case InputFieldType_Boolean() when boolean != null:
return boolean();case InputFieldType_File() when file != null:
return file(_that.policy);case InputFieldType_Select() when select != null:
return select(_that.options);case InputFieldType_Multiline() when multiline != null:
return multiline();case InputFieldType_MultiOptions() when multiOptions != null:
return multiOptions(_that.options);case InputFieldType_DateTime() when dateTime != null:
return dateTime();case InputFieldType_Markdown() when markdown != null:
return markdown();case InputFieldType_FilePath() when filePath != null:
return filePath();case InputFieldType_Url() when url != null:
return url();case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function()  text,required TResult Function()  number,required TResult Function()  integer,required TResult Function()  boolean,required TResult Function( FileInputPolicyDto policy)  file,required TResult Function( List<ChoiceOptionDto> options)  select,required TResult Function()  multiline,required TResult Function( List<ChoiceOptionDto> options)  multiOptions,required TResult Function()  dateTime,required TResult Function()  markdown,required TResult Function()  filePath,required TResult Function()  url,}) {final _that = this;
switch (_that) {
case InputFieldType_Text():
return text();case InputFieldType_Number():
return number();case InputFieldType_Integer():
return integer();case InputFieldType_Boolean():
return boolean();case InputFieldType_File():
return file(_that.policy);case InputFieldType_Select():
return select(_that.options);case InputFieldType_Multiline():
return multiline();case InputFieldType_MultiOptions():
return multiOptions(_that.options);case InputFieldType_DateTime():
return dateTime();case InputFieldType_Markdown():
return markdown();case InputFieldType_FilePath():
return filePath();case InputFieldType_Url():
return url();}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function()?  text,TResult? Function()?  number,TResult? Function()?  integer,TResult? Function()?  boolean,TResult? Function( FileInputPolicyDto policy)?  file,TResult? Function( List<ChoiceOptionDto> options)?  select,TResult? Function()?  multiline,TResult? Function( List<ChoiceOptionDto> options)?  multiOptions,TResult? Function()?  dateTime,TResult? Function()?  markdown,TResult? Function()?  filePath,TResult? Function()?  url,}) {final _that = this;
switch (_that) {
case InputFieldType_Text() when text != null:
return text();case InputFieldType_Number() when number != null:
return number();case InputFieldType_Integer() when integer != null:
return integer();case InputFieldType_Boolean() when boolean != null:
return boolean();case InputFieldType_File() when file != null:
return file(_that.policy);case InputFieldType_Select() when select != null:
return select(_that.options);case InputFieldType_Multiline() when multiline != null:
return multiline();case InputFieldType_MultiOptions() when multiOptions != null:
return multiOptions(_that.options);case InputFieldType_DateTime() when dateTime != null:
return dateTime();case InputFieldType_Markdown() when markdown != null:
return markdown();case InputFieldType_FilePath() when filePath != null:
return filePath();case InputFieldType_Url() when url != null:
return url();case _:
  return null;

}
}

}

/// @nodoc


class InputFieldType_Text extends InputFieldType {
  const InputFieldType_Text(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is InputFieldType_Text);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'InputFieldType.text()';
}


}




/// @nodoc


class InputFieldType_Number extends InputFieldType {
  const InputFieldType_Number(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is InputFieldType_Number);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'InputFieldType.number()';
}


}




/// @nodoc


class InputFieldType_Integer extends InputFieldType {
  const InputFieldType_Integer(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is InputFieldType_Integer);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'InputFieldType.integer()';
}


}




/// @nodoc


class InputFieldType_Boolean extends InputFieldType {
  const InputFieldType_Boolean(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is InputFieldType_Boolean);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'InputFieldType.boolean()';
}


}




/// @nodoc


class InputFieldType_File extends InputFieldType {
  const InputFieldType_File({required this.policy}): super._();


 final  FileInputPolicyDto policy;

/// Create a copy of InputFieldType
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$InputFieldType_FileCopyWith<InputFieldType_File> get copyWith => _$InputFieldType_FileCopyWithImpl<InputFieldType_File>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is InputFieldType_File&&(identical(other.policy, policy) || other.policy == policy));
}


@override
int get hashCode => Object.hash(runtimeType,policy);

@override
String toString() {
  return 'InputFieldType.file(policy: $policy)';
}


}

/// @nodoc
abstract mixin class $InputFieldType_FileCopyWith<$Res> implements $InputFieldTypeCopyWith<$Res> {
  factory $InputFieldType_FileCopyWith(InputFieldType_File value, $Res Function(InputFieldType_File) _then) = _$InputFieldType_FileCopyWithImpl;
@useResult
$Res call({
 FileInputPolicyDto policy
});




}
/// @nodoc
class _$InputFieldType_FileCopyWithImpl<$Res>
    implements $InputFieldType_FileCopyWith<$Res> {
  _$InputFieldType_FileCopyWithImpl(this._self, this._then);

  final InputFieldType_File _self;
  final $Res Function(InputFieldType_File) _then;

/// Create a copy of InputFieldType
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? policy = null,}) {
  return _then(InputFieldType_File(
policy: null == policy ? _self.policy : policy // ignore: cast_nullable_to_non_nullable
as FileInputPolicyDto,
  ));
}


}

/// @nodoc


class InputFieldType_Select extends InputFieldType {
  const InputFieldType_Select({required final  List<ChoiceOptionDto> options}): _options = options,super._();


 final  List<ChoiceOptionDto> _options;
 List<ChoiceOptionDto> get options {
  if (_options is EqualUnmodifiableListView) return _options;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_options);
}


/// Create a copy of InputFieldType
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$InputFieldType_SelectCopyWith<InputFieldType_Select> get copyWith => _$InputFieldType_SelectCopyWithImpl<InputFieldType_Select>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is InputFieldType_Select&&const DeepCollectionEquality().equals(other._options, _options));
}


@override
int get hashCode => Object.hash(runtimeType,const DeepCollectionEquality().hash(_options));

@override
String toString() {
  return 'InputFieldType.select(options: $options)';
}


}

/// @nodoc
abstract mixin class $InputFieldType_SelectCopyWith<$Res> implements $InputFieldTypeCopyWith<$Res> {
  factory $InputFieldType_SelectCopyWith(InputFieldType_Select value, $Res Function(InputFieldType_Select) _then) = _$InputFieldType_SelectCopyWithImpl;
@useResult
$Res call({
 List<ChoiceOptionDto> options
});




}
/// @nodoc
class _$InputFieldType_SelectCopyWithImpl<$Res>
    implements $InputFieldType_SelectCopyWith<$Res> {
  _$InputFieldType_SelectCopyWithImpl(this._self, this._then);

  final InputFieldType_Select _self;
  final $Res Function(InputFieldType_Select) _then;

/// Create a copy of InputFieldType
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? options = null,}) {
  return _then(InputFieldType_Select(
options: null == options ? _self._options : options // ignore: cast_nullable_to_non_nullable
as List<ChoiceOptionDto>,
  ));
}


}

/// @nodoc


class InputFieldType_Multiline extends InputFieldType {
  const InputFieldType_Multiline(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is InputFieldType_Multiline);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'InputFieldType.multiline()';
}


}




/// @nodoc


class InputFieldType_MultiOptions extends InputFieldType {
  const InputFieldType_MultiOptions({required final  List<ChoiceOptionDto> options}): _options = options,super._();


 final  List<ChoiceOptionDto> _options;
 List<ChoiceOptionDto> get options {
  if (_options is EqualUnmodifiableListView) return _options;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_options);
}


/// Create a copy of InputFieldType
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$InputFieldType_MultiOptionsCopyWith<InputFieldType_MultiOptions> get copyWith => _$InputFieldType_MultiOptionsCopyWithImpl<InputFieldType_MultiOptions>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is InputFieldType_MultiOptions&&const DeepCollectionEquality().equals(other._options, _options));
}


@override
int get hashCode => Object.hash(runtimeType,const DeepCollectionEquality().hash(_options));

@override
String toString() {
  return 'InputFieldType.multiOptions(options: $options)';
}


}

/// @nodoc
abstract mixin class $InputFieldType_MultiOptionsCopyWith<$Res> implements $InputFieldTypeCopyWith<$Res> {
  factory $InputFieldType_MultiOptionsCopyWith(InputFieldType_MultiOptions value, $Res Function(InputFieldType_MultiOptions) _then) = _$InputFieldType_MultiOptionsCopyWithImpl;
@useResult
$Res call({
 List<ChoiceOptionDto> options
});




}
/// @nodoc
class _$InputFieldType_MultiOptionsCopyWithImpl<$Res>
    implements $InputFieldType_MultiOptionsCopyWith<$Res> {
  _$InputFieldType_MultiOptionsCopyWithImpl(this._self, this._then);

  final InputFieldType_MultiOptions _self;
  final $Res Function(InputFieldType_MultiOptions) _then;

/// Create a copy of InputFieldType
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? options = null,}) {
  return _then(InputFieldType_MultiOptions(
options: null == options ? _self._options : options // ignore: cast_nullable_to_non_nullable
as List<ChoiceOptionDto>,
  ));
}


}

/// @nodoc


class InputFieldType_DateTime extends InputFieldType {
  const InputFieldType_DateTime(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is InputFieldType_DateTime);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'InputFieldType.dateTime()';
}


}




/// @nodoc


class InputFieldType_Markdown extends InputFieldType {
  const InputFieldType_Markdown(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is InputFieldType_Markdown);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'InputFieldType.markdown()';
}


}




/// @nodoc


class InputFieldType_FilePath extends InputFieldType {
  const InputFieldType_FilePath(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is InputFieldType_FilePath);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'InputFieldType.filePath()';
}


}




/// @nodoc


class InputFieldType_Url extends InputFieldType {
  const InputFieldType_Url(): super._();







@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is InputFieldType_Url);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'InputFieldType.url()';
}


}




// dart format on
