import { DropdownMenu as DropdownMenuPrimitive } from 'bits-ui';
import Content from './dropdown-menu-content.svelte';
import Item from './dropdown-menu-item.svelte';
import Separator from './dropdown-menu-separator.svelte';
import Label from './dropdown-menu-label.svelte';

const Root = DropdownMenuPrimitive.Root;
const Trigger = DropdownMenuPrimitive.Trigger;
const Group = DropdownMenuPrimitive.Group;
const Sub = DropdownMenuPrimitive.Sub;
const RadioGroup = DropdownMenuPrimitive.RadioGroup;

export {
  Root,
  Root as DropdownMenu,
  Content,
  Content as DropdownMenuContent,
  Group,
  Group as DropdownMenuGroup,
  Item,
  Item as DropdownMenuItem,
  Label,
  Label as DropdownMenuLabel,
  RadioGroup,
  RadioGroup as DropdownMenuRadioGroup,
  Separator,
  Separator as DropdownMenuSeparator,
  Sub,
  Sub as DropdownMenuSub,
  Trigger,
  Trigger as DropdownMenuTrigger,
};
