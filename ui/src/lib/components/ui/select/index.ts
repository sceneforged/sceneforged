import { Select as SelectPrimitive } from 'bits-ui';
import Content from './select-content.svelte';
import Item from './select-item.svelte';
import Trigger from './select-trigger.svelte';
import Value from './select-value.svelte';

const Root = SelectPrimitive.Root;
const Group = SelectPrimitive.Group;
const GroupHeading = SelectPrimitive.GroupHeading;

export {
  Root,
  Root as Select,
  Content,
  Content as SelectContent,
  Group,
  Group as SelectGroup,
  GroupHeading,
  GroupHeading as SelectGroupHeading,
  Item,
  Item as SelectItem,
  Trigger,
  Trigger as SelectTrigger,
  Value,
  Value as SelectValue,
};
