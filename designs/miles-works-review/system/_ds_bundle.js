/* @ds-bundle: {"format":3,"namespace":"System_bbc5e6","components":[{"name":"Button","sourcePath":"components/Button/Button.jsx"},{"name":"Choice","sourcePath":"components/Choice/Choice.jsx"},{"name":"ConnectionRow","sourcePath":"components/ConnectionRow/ConnectionRow.jsx"},{"name":"DeliveryTimeline","sourcePath":"components/DeliveryTimeline/DeliveryTimeline.jsx"},{"name":"Dialog","sourcePath":"components/Dialog/Dialog.jsx"},{"name":"Field","sourcePath":"components/Field/Field.jsx"},{"name":"NavigationItem","sourcePath":"components/NavigationItem/NavigationItem.jsx"},{"name":"Notice","sourcePath":"components/Notice/Notice.jsx"},{"name":"PermissionSummary","sourcePath":"components/PermissionSummary/PermissionSummary.jsx"},{"name":"SelectField","sourcePath":"components/SelectField/SelectField.jsx"},{"name":"SpacePicker","sourcePath":"components/SpacePicker/SpacePicker.jsx"},{"name":"StatusTag","sourcePath":"components/StatusTag/StatusTag.jsx"}],"sourceHashes":{"components/Button/Button.jsx":"2a189538c65a","components/Choice/Choice.jsx":"7475e3c114ec","components/ConnectionRow/ConnectionRow.jsx":"efe20e0e7085","components/DeliveryTimeline/DeliveryTimeline.jsx":"1555c2fb2a1f","components/Dialog/Dialog.jsx":"6e5712f69d75","components/Field/Field.jsx":"d0695bf080b7","components/NavigationItem/NavigationItem.jsx":"386fe034f567","components/Notice/Notice.jsx":"b5e78fd04687","components/PermissionSummary/PermissionSummary.jsx":"8a93699d22f0","components/SelectField/SelectField.jsx":"27b006305571","components/SpacePicker/SpacePicker.jsx":"3c0882d6d0d8","components/StatusTag/StatusTag.jsx":"0686b8d3b632"},"inlinedExternals":[],"unexposedExports":[]} */

(() => {

const __ds_ns = (window.System_bbc5e6 = window.System_bbc5e6 || {});

const __ds_scope = {};

(__ds_ns.__errors = __ds_ns.__errors || []);

// components/Button/Button.jsx
try { (() => {
function Button({
  label,
  variant = 'primary',
  disabled = false,
  onClick
}) {
  return /*#__PURE__*/React.createElement("button", {
    type: "button",
    className: variant === 'primary' ? '' : variant,
    disabled: disabled,
    onClick: onClick
  }, label);
}
Object.assign(__ds_scope, { Button });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/Button/Button.jsx", error: String((e && e.message) || e) }); }

// components/Choice/Choice.jsx
try { (() => {
function Choice({
  label,
  description,
  checked,
  disabled = false,
  onChange
}) {
  return /*#__PURE__*/React.createElement("label", {
    className: "check"
  }, /*#__PURE__*/React.createElement("input", {
    type: "checkbox",
    checked: checked,
    disabled: disabled,
    onChange: onChange
  }), /*#__PURE__*/React.createElement("span", null, label, description && /*#__PURE__*/React.createElement("p", null, description)));
}
Object.assign(__ds_scope, { Choice });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/Choice/Choice.jsx", error: String((e && e.message) || e) }); }

// components/ConnectionRow/ConnectionRow.jsx
try { (() => {
function ConnectionRow({
  name,
  detail,
  status,
  onManage
}) {
  return /*#__PURE__*/React.createElement("div", {
    className: "row",
    style: {
      padding: 'var(--space-4)',
      borderBottom: '1px solid var(--border)'
    }
  }, /*#__PURE__*/React.createElement("div", null, /*#__PURE__*/React.createElement("strong", null, name), /*#__PURE__*/React.createElement("p", {
    className: "small",
    style: {
      margin: 0
    }
  }, detail)), /*#__PURE__*/React.createElement("span", {
    className: "tag"
  }, status), /*#__PURE__*/React.createElement("button", {
    type: "button",
    className: "secondary",
    onClick: onManage
  }, "\u7BA1\u7406"));
}
Object.assign(__ds_scope, { ConnectionRow });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/ConnectionRow/ConnectionRow.jsx", error: String((e && e.message) || e) }); }

// components/DeliveryTimeline/DeliveryTimeline.jsx
try { (() => {
function DeliveryTimeline({
  stage = 'stored',
  unknown = false
}) {
  const items = ['stored', 'notified', 'received', 'processed'];
  const labels = ['已存入云端', '通知已接受', '助手已接收', '已处理'];
  return /*#__PURE__*/React.createElement("ol", {
    className: "timeline"
  }, items.map((s, i) => /*#__PURE__*/React.createElement("li", {
    key: s,
    className: i <= items.indexOf(stage) ? 'done' : i === 1 && unknown ? 'current' : ''
  }, /*#__PURE__*/React.createElement("strong", null, i === 1 && unknown ? '通知结果待核对' : labels[i]), /*#__PURE__*/React.createElement("span", null, i === 1 && unknown ? '不自动重发，先核对记录' : i <= items.indexOf(stage) ? '已确认' : '等待确认'))));
}
Object.assign(__ds_scope, { DeliveryTimeline });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/DeliveryTimeline/DeliveryTimeline.jsx", error: String((e && e.message) || e) }); }

// components/Dialog/Dialog.jsx
try { (() => {
function Dialog({
  title,
  children,
  onClose
}) {
  const ref = React.useRef(null);
  const titleId = React.useId();
  React.useEffect(() => {
    const d = ref.current;
    if (d && !d.open) d.showModal();
    return () => {
      if (d?.open) d.close();
    };
  }, []);
  return /*#__PURE__*/React.createElement("dialog", {
    ref: ref,
    "aria-labelledby": titleId,
    onClose: onClose
  }, /*#__PURE__*/React.createElement("div", {
    className: "row"
  }, /*#__PURE__*/React.createElement("h2", {
    id: titleId
  }, title), /*#__PURE__*/React.createElement("button", {
    type: "button",
    className: "ghost",
    onClick: () => ref.current?.close()
  }, "\u5173\u95ED")), children);
}
Object.assign(__ds_scope, { Dialog });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/Dialog/Dialog.jsx", error: String((e && e.message) || e) }); }

// components/Field/Field.jsx
try { (() => {
function Field({
  id,
  label,
  value,
  error,
  onChange
}) {
  return /*#__PURE__*/React.createElement("label", {
    htmlFor: id
  }, label, /*#__PURE__*/React.createElement("input", {
    id: id,
    value: value,
    onChange: onChange,
    "aria-invalid": !!error,
    "aria-describedby": error ? id + '-error' : undefined
  }), error && /*#__PURE__*/React.createElement("span", {
    id: id + '-error',
    role: "status",
    style: {
      color: 'var(--danger)',
      fontSize: 'var(--type-xs)'
    }
  }, error));
}
Object.assign(__ds_scope, { Field });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/Field/Field.jsx", error: String((e && e.message) || e) }); }

// components/NavigationItem/NavigationItem.jsx
try { (() => {
function NavigationItem({
  label,
  current = false,
  onClick
}) {
  return /*#__PURE__*/React.createElement("button", {
    type: "button",
    className: current ? 'secondary' : 'ghost',
    "aria-current": current ? 'page' : undefined,
    onClick: onClick
  }, label);
}
Object.assign(__ds_scope, { NavigationItem });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/NavigationItem/NavigationItem.jsx", error: String((e && e.message) || e) }); }

// components/Notice/Notice.jsx
try { (() => {
function Notice({
  children,
  tone = 'neutral',
  urgent = false
}) {
  return /*#__PURE__*/React.createElement("div", {
    className: 'notice ' + (tone === 'neutral' ? '' : tone),
    role: urgent ? 'alert' : undefined
  }, children);
}
Object.assign(__ds_scope, { Notice });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/Notice/Notice.jsx", error: String((e && e.message) || e) }); }

// components/PermissionSummary/PermissionSummary.jsx
try { (() => {
function PermissionSummary({
  space,
  sensitive = false
}) {
  return /*#__PURE__*/React.createElement("div", {
    className: 'notice ' + (sensitive ? 'warning' : '')
  }, /*#__PURE__*/React.createElement("strong", null, sensitive ? '密钥读取' : 'Space 成员访问', " \xB7 ", space), /*#__PURE__*/React.createElement("p", {
    className: "small",
    style: {
      margin: 'var(--space-2) 0 0'
    }
  }, sensitive ? '所选条目的值会返回给助手，进入宿主与模型上下文。' : '可发送消息、读取该身份的收发消息，并读写本 Space 的共享资料和封存附件。'));
}
Object.assign(__ds_scope, { PermissionSummary });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/PermissionSummary/PermissionSummary.jsx", error: String((e && e.message) || e) }); }

// components/SelectField/SelectField.jsx
try { (() => {
function SelectField({
  id,
  label,
  options,
  value,
  onChange
}) {
  return /*#__PURE__*/React.createElement("label", {
    htmlFor: id
  }, label, /*#__PURE__*/React.createElement("select", {
    id: id,
    value: value,
    onChange: onChange
  }, options.map(o => /*#__PURE__*/React.createElement("option", {
    key: o
  }, o))));
}
Object.assign(__ds_scope, { SelectField });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/SelectField/SelectField.jsx", error: String((e && e.message) || e) }); }

// components/SpacePicker/SpacePicker.jsx
try { (() => {
function SpacePicker({
  spaces,
  selected,
  onChange
}) {
  return /*#__PURE__*/React.createElement("label", null, "\u5F53\u524D Space", /*#__PURE__*/React.createElement("select", {
    value: selected,
    onChange: onChange
  }, spaces.map(s => /*#__PURE__*/React.createElement("option", {
    key: s
  }, s))), /*#__PURE__*/React.createElement("span", {
    className: "small muted"
  }, "\u5207\u6362\u5DE5\u4F5C\u53F0\u4E0D\u4F1A\u53D8\u66F4\u5DF2\u6709\u52A9\u624B\u6388\u6743\u3002"));
}
Object.assign(__ds_scope, { SpacePicker });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/SpacePicker/SpacePicker.jsx", error: String((e && e.message) || e) }); }

// components/StatusTag/StatusTag.jsx
try { (() => {
function StatusTag({
  label,
  tone = 'neutral'
}) {
  return /*#__PURE__*/React.createElement("span", {
    className: 'tag ' + (tone === 'neutral' ? '' : tone)
  }, label);
}
Object.assign(__ds_scope, { StatusTag });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/StatusTag/StatusTag.jsx", error: String((e && e.message) || e) }); }

__ds_ns.Button = __ds_scope.Button;

__ds_ns.Choice = __ds_scope.Choice;

__ds_ns.ConnectionRow = __ds_scope.ConnectionRow;

__ds_ns.DeliveryTimeline = __ds_scope.DeliveryTimeline;

__ds_ns.Dialog = __ds_scope.Dialog;

__ds_ns.Field = __ds_scope.Field;

__ds_ns.NavigationItem = __ds_scope.NavigationItem;

__ds_ns.Notice = __ds_scope.Notice;

__ds_ns.PermissionSummary = __ds_scope.PermissionSummary;

__ds_ns.SelectField = __ds_scope.SelectField;

__ds_ns.SpacePicker = __ds_scope.SpacePicker;

__ds_ns.StatusTag = __ds_scope.StatusTag;

})();
