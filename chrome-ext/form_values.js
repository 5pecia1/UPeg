'use strict';

// Popup form value semantics. Keeping these pure makes the distinction
// between an omitted field and explicit false/zero/empty text testable
// without a browser DOM.
const UpegFormValues = (() => {
  const FIELD_TYPE = Object.freeze({
    BOOLEAN: 'boolean',
    NUMBER: 'number',
    INTEGER: 'integer',
    JSON: 'json',
  });

  function initialValue(preset, name, schema) {
    if (preset && Object.hasOwn(preset, name)) return { hasValue: true, value: preset[name] };
    if (schema && Object.hasOwn(schema, 'default')) return { hasValue: true, value: schema.default };
    return { hasValue: false, value: undefined };
  }

  function scalarValue({ fieldType, raw, checked, required, hasInitial, touched, initialValue }) {
    if (fieldType === FIELD_TYPE.BOOLEAN) {
      if (hasInitial && typeof initialValue !== 'boolean' && !touched) {
        return { ok: true, include: true, value: initialValue };
      }
      return {
        ok: true,
        include: required || hasInitial || touched,
        value: checked,
      };
    }

    const trimmed = raw.trim();
    if (fieldType === FIELD_TYPE.NUMBER || fieldType === FIELD_TYPE.INTEGER) {
      if (trimmed.length === 0) {
        return required || hasInitial || touched
          ? { ok: false, reason: 'number_required' }
          : { ok: true, include: false };
      }
      const value = Number(trimmed);
      return !Number.isFinite(value) || (fieldType === FIELD_TYPE.INTEGER && !Number.isInteger(value))
        ? { ok: false, reason: 'number_invalid' }
        : { ok: true, include: true, value };
    }

    if (fieldType === FIELD_TYPE.JSON) {
      if (trimmed.length === 0) {
        return required || hasInitial || touched
          ? { ok: false, reason: 'json_required' }
          : { ok: true, include: false };
      }
      try {
        return { ok: true, include: true, value: JSON.parse(trimmed) };
      } catch {
        return { ok: false, reason: 'json_invalid' };
      }
    }

    if (raw.length === 0) {
      if (required && !hasInitial) return { ok: false, reason: 'required' };
      return { ok: true, include: hasInitial || touched, value: raw };
    }
    return { ok: true, include: true, value: raw };
  }

  return Object.freeze({ FIELD_TYPE, initialValue, scalarValue });
})();

if (typeof module === 'object' && module.exports) {
  module.exports = UpegFormValues;
}
