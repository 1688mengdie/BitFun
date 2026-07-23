import transportAndGeneralChatUnitTest from './TransportAndGeneralChatUnit.test';
import conversationStateUnitTest from './ConversationStateUnit.test';
import lifecycleUnitTest from './LifecycleUnit.test';
import remoteControllersUnitTest from './RemoteControllersUnit.test';

export default function localUnitTest() {
  transportAndGeneralChatUnitTest();
  conversationStateUnitTest();
  lifecycleUnitTest();
  remoteControllersUnitTest();
}
