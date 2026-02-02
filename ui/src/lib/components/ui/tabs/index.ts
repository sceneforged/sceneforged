import { Tabs as TabsPrimitive } from 'bits-ui';
import Content from './tabs-content.svelte';
import List from './tabs-list.svelte';
import Trigger from './tabs-trigger.svelte';

const Root = TabsPrimitive.Root;

export {
  Root,
  Root as Tabs,
  Content,
  Content as TabsContent,
  List,
  List as TabsList,
  Trigger,
  Trigger as TabsTrigger,
};
