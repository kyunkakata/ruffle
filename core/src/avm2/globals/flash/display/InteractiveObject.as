package flash.display {
    import __ruffle__.stub_setter;

    import flash.accessibility.AccessibilityImplementation;
	import flash.ui.ContextMenu;

	[Ruffle(NativeInstanceInit)]
	public class InteractiveObject extends DisplayObject {
	    private var _accessibilityImpl:AccessibilityImplementation = null;

		public function InteractiveObject() {
			throw new Error("You cannot directly construct InteractiveObject.")
		}

		public function get accessibilityImplementation():AccessibilityImplementation {
		    return this._accessibilityImpl;
		}
		public function set accessibilityImplementation(value:AccessibilityImplementation):void {
		    stub_setter("flash.display.InteractiveObject", "accessibilityImplementation");
		    this._accessibilityImpl = value;
		}

		public native function get mouseEnabled():Boolean;
		public native function set mouseEnabled(value:Boolean):void;

		public native function get doubleClickEnabled():Boolean;
		public native function set doubleClickEnabled(value:Boolean):void;

		public native function get contextMenu():ContextMenu;
		public native function set contextMenu(cm:ContextMenu):void;

		public native function get tabEnabled():Boolean;
		public native function set tabEnabled(value:Boolean):void;

		public native function get tabIndex():int;
		public native function set tabIndex(index:int):void;

		public native function get focusRect():Object;
		public native function set focusRect(value:Object):void;
	}
}
