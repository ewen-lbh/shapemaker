while true
    set id (nanoid -s 10)
    qrencode "https://shapemaker.ewen.works/found/$id" -o street/$id-qr.png
    QRCODE_NAME=street/$id just example-image out.svg "--objects-count 5..15"
    if test (read || echo "n") = "y"
        cp out.svg street/$id.svg
        resvg --width 2000 out.svg street/$id.png
        echo resvg --width 2000 street/$id.svg street/$id.png
        echo saved street/$id.svg \| street/$id.png
    else
        rm street/$id-qr.png
    end
end
