package flash.text.engine {
    import __ruffle__.stub_getter;
    import __ruffle__.stub_setter;
    import __ruffle__.stub_method;

    import flash.display.DisplayObjectContainer;
    import flash.geom.Rectangle;

    // FIXME: None of the DisplayObjectContainer methods actually work on
    // the TextLine class in Ruffle, despite the methods working fine in FP-
    // however, it's unlikely that SWFs will actually attempt to add children
    // to a TextLine.
    [Ruffle(NativeInstanceInit)]
    public final class TextLine extends DisplayObjectContainer {
        internal var _specifiedWidth:Number = 0.0;
        internal var _textBlock:TextBlock = null;
        internal var _rawTextLength:int = 0;
        internal var _validity:String = "valid";

        public static const MAX_LINE_WIDTH:int = 1000000;

        public var userData;

        public function TextLine() {
            throw new ArgumentError("Error #2012: TextLine$ class cannot be instantiated.", 2012);
        }

        public function get rawTextLength():int {
            return this._rawTextLength;
        }

        public function get textBlockBeginIndex():int {
            stub_getter("flash.text.engine.TextLine", "textBlockBeginIndex");
            return 0;
        }

        public function get specifiedWidth():Number {
            return this._specifiedWidth;
        }

        public function get textBlock():TextBlock {
            return this._textBlock;
        }

        public function get ascent():Number {
            stub_getter("flash.text.engine.TextLine", "ascent");
            return 12.0;
        }

        public function get descent():Number {
            stub_getter("flash.text.engine.TextLine", "descent");
            return 3.0;
        }

        public function get unjustifiedTextWidth():Number {
            stub_getter("flash.text.engine.TextLine", "unjustifiedTextWidth");
            return this._specifiedWidth;
        }

        public function get textWidth():Number {
            stub_getter("flash.text.engine.TextLine", "textWidth");
            return this._specifiedWidth;
        }

        public function get textHeight():Number {
            stub_getter("flash.text.engine.TextLine", "textHeight");
            return 15.0;
        }

        public function get validity():String {
            stub_getter("flash.text.engine.TextLine", "validity");
            return this._validity;
        }

        public function set validity(value:String):void {
            stub_setter("flash.text.engine.TextLine", "validity");
            this._validity = value;
        }

        public function get hasGraphicElement():Boolean {
            stub_getter("flash.text.engine.TextLine", "hasGraphicElement");
            return false;
        }

        public function get atomCount():int {
            stub_getter("flash.text.engine.TextLine", "atomCount");
            return this._rawTextLength;
        }

        public function get nextLine():TextLine {
            return null;
        }

        public function get previousLine():TextLine {
            return null;
        }

        public function getBaselinePosition(baseline:String):Number {
            stub_method("flash.text.engine.TextLine", "getBaselinePosition");
            return 0.0;
        }

        public function hasTabs():Boolean {
            stub_getter("flash.text.engine.TextLine", "hasTabs");
            return false;
        }

        public function getAtomIndexAtPoint(stageX:Number, stageY:Number):int {
            stub_method("flash.text.engine.TextLine", "getAtomIndexAtPoint");
            return -1;
        }

        public function getAtomBounds(index:int):Rectangle {
            stub_method("flash.text.engine.TextLine", "getAtomBounds");
            return new Rectangle(0, 0, 0, 0);
        }

        // This function does nothing in Flash Player 32
        public function flushAtomData():void { }
    }
}
